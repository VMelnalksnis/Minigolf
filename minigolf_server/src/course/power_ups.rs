use {
    crate::{
        HoleState, LastPlayerPosition, PlayingSystems, ValidPlayerInput,
        course::{
            Configuration, CurrentHole, HoleSensor, HoleWalls,
            entities::{BUMPER_HEIGHT, BUMPER_RADIUS},
            setup::{SpawnBlackHoleBumper, SpawnBumper},
        },
    },
    avian3d::{
        math::{Quaternion, Scalar, Vector},
        prelude::*,
    },
    bevy::{ecs::system::SystemParam, prelude::*},
    minigolf::{CourseEffect, Player, PlayerInput, PlayerPowerUps, PowerUp, PowerUpType},
};

pub(crate) struct PowerUpPlugin;

impl Plugin for PowerUpPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<HoleMagnetPowerUp>();
        app.register_type::<StickyBall>();
        app.register_type::<AwakeAtStepStart>();
        app.register_required_components::<Player, AwakeAtStepStart>();
        app.register_type::<ChipShotMarker>();

        // Runs after the solver, like collision events. Checked every step instead of on
        // `CollisionStart`, because all walls of a hole are a single collider, so hitting a wall
        // while already touching another one does not start a new collision.
        app.add_systems(
            PhysicsSchedule,
            (
                record_awake_at_step_start.in_set(PhysicsStepSystems::First),
                apply_sticky_effects
                    .in_set(PhysicsStepSystems::Finalize)
                    .run_if(in_state(HoleState::Playing)),
            ),
        );

        app.add_systems(Update, apply_power_ups.in_set(PlayingSystems));

        app.add_systems(
            FixedUpdate,
            (
                handle_power_up_sensors,
                apply_winds,
                apply_hole_magnet,
                remove_hole_magnet,
            )
                .in_set(PlayingSystems),
        );

        app.add_systems(
            OnEnter(HoleState::Completed),
            (remove_sticky_ball, despawn_winds),
        );
    }
}

/// Indicates that [minigolf::PowerUpType::ChipShot] should apply to the next hit for the player.
#[derive(Component, Reflect, Debug)]
pub(crate) struct ChipShotMarker;

fn apply_power_ups(
    mut reader: MessageReader<ValidPlayerInput>,
    current_hole: Res<CurrentHole>,
    mut commands: Commands,
    players: Query<Entity, With<Player>>,
    hole_walls: Query<(Entity, &HoleWalls)>,
    bumper_placement: BumperPlacement,
    mut power_ups: Query<&mut PlayerPowerUps>,
) {
    for &ValidPlayerInput { input, player } in reader.read() {
        match input {
            PlayerInput::Move(_) => {}

            PlayerInput::Teleport(translation) => {
                let mut vec = Vector::from(translation);
                vec.y = vec.y + 0.05;

                commands.entity(player).insert(Position(vec));
            }

            PlayerInput::HoleMagnet => {
                commands.entity(player).insert(HoleMagnetPowerUp);
            }

            PlayerInput::ChipShot => {
                commands.entity(player).insert(ChipShotMarker);
            }

            PlayerInput::StickyBall => {
                for other_player in players.iter().filter(|e| *e != player) {
                    commands.entity(other_player).insert(StickyBall);
                }
            }

            PlayerInput::Bumper(translation) => match bumper_placement.place(translation) {
                Some(transform) => commands.trigger(SpawnBumper::with_hits(transform)),
                None => refund_power_up(player, PowerUpType::Bumper, &mut power_ups),
            },

            PlayerInput::BlackHoleBumper(translation) => {
                match bumper_placement.place(translation) {
                    Some(transform) => {
                        commands.trigger(SpawnBlackHoleBumper::with_hits(transform))
                    }
                    None => refund_power_up(player, PowerUpType::BlackHoleBumper, &mut power_ups),
                }
            }

            PlayerInput::Wind(direction) => {
                let direction = direction.normalize();
                commands.spawn((Name::new("Wind"), Wind { direction }));
            }

            PlayerInput::StickyWalls => {
                let walls = hole_walls
                    .iter()
                    .filter(|(_, w)| w.hole_entity == current_hole.hole_entity)
                    .map(|(e, _)| e)
                    .next()
                    .unwrap();

                commands.entity(walls).insert(CourseEffect::StickyWalls);
            }

            PlayerInput::IceRink => {
                commands.entity(current_hole.hole_entity).insert((
                    Friction::new(0.01).with_combine_rule(CoefficientCombine::Min),
                    CourseEffect::IceRink,
                ));
            }

            _ => {
                warn!("Unhandled player input type {:?}", input);
            }
        }
    }
}

/// Gives back a power up that was used for an input that could not be applied.
fn refund_power_up(
    player: Entity,
    power_up: PowerUpType,
    power_ups: &mut Query<&mut PlayerPowerUps>,
) {
    warn!(
        "Could not apply power up {:?} for player {:?}, refunding it",
        power_up, player
    );

    let Ok(mut player_power_ups) = power_ups.get_mut(player) else {
        return;
    };

    player_power_ups.refund_power_up(power_up);
}

/// Distance above and below the requested position in which the floor of the current hole is
/// searched for when placing a bumper.
const BUMPER_FLOOR_SEARCH_DISTANCE: Scalar = 0.05;

#[derive(SystemParam)]
struct BumperPlacement<'w, 's> {
    current_hole: Res<'w, CurrentHole>,
    spatial_query: SpatialQuery<'w, 's>,
    transforms: Query<'w, 's, &'static GlobalTransform>,
    /// Sensors that a bumper can be placed in, e.g. power ups and the bounds of the hole, but not
    /// the hole itself.
    ignored_sensors: Query<'w, 's, (), (With<Sensor>, Without<HoleSensor>)>,
}

impl BumperPlacement<'_, '_> {
    /// Gets the transform, relative to the current hole, of a bumper placed at the requested world
    /// position.
    ///
    /// Returns `None` if the position is not on the floor of the current hole, or the bumper would
    /// overlap with a ball, the walls, another bumper or the hole.
    fn place(&self, translation: Vec3) -> Option<Transform> {
        let hole_entity = self.current_hole.hole_entity;
        let hole_transform = self.transforms.get(hole_entity).ok()?;

        // The position comes from the client, so find the floor below it instead of trusting it.
        let origin = Vector::from(translation) + Vector::Y * BUMPER_FLOOR_SEARCH_DISTANCE;
        let floor_hit = self.spatial_query.cast_ray_predicate(
            origin,
            Dir3::NEG_Y,
            BUMPER_FLOOR_SEARCH_DISTANCE * 2.0,
            true,
            &SpatialQueryFilter::default(),
            &|entity| entity == hole_entity,
        )?;

        let center = origin - Vector::Y * (floor_hit.distance - BUMPER_HEIGHT / 2.0);

        // Shorter than the bumper, so that it does not touch the floor or what is below it.
        let shape = Collider::cylinder(BUMPER_RADIUS, BUMPER_HEIGHT / 2.0);
        let blocked = self
            .spatial_query
            .shape_intersections(
                &shape,
                center,
                Quaternion::IDENTITY,
                &SpatialQueryFilter::default(),
            )
            .into_iter()
            .any(|entity| entity != hole_entity && !self.ignored_sensors.contains(entity));

        if blocked {
            return None;
        }

        let world = GlobalTransform::from(Transform::from_translation(center.as_vec3()));
        Some(world.reparented_to(hole_transform))
    }
}

fn handle_power_up_sensors(
    power_ups: Query<(Entity, &PowerUp, &CollidingEntities), Changed<CollidingEntities>>,
    mut players: Query<(Entity, &mut PlayerPowerUps), With<Player>>,
    mut commands: Commands,
) {
    for (power_up_entity, power_up, collisions) in power_ups.iter() {
        for (player, mut player_power_ups) in &mut players {
            if !collisions.contains(&player) {
                continue;
            }

            info!(
                "Player {:?} collided with power up {:?}",
                player, power_up_entity
            );

            match player_power_ups.add_power_up(power_up.power_up.clone()) {
                Ok(_) => {
                    info!(
                        "Player {:?} picked up power up {:?}",
                        player, power_up_entity
                    );

                    commands.entity(power_up_entity).despawn();
                }
                Err(_) => {
                    info!(
                        "Player {:?} could not pick up power up {:?}",
                        player, power_up_entity
                    );
                }
            }
        }
    }
}

#[derive(Component, Reflect, Debug)]
struct Wind {
    direction: Vec2,
}

fn apply_winds(
    winds: Query<&Wind>,
    players: Query<Entity, With<Player>>,
    holes: Query<&CollidingEntities, With<HoleSensor>>,
    config: Res<Configuration>,
    mut forces: Query<Forces>,
) {
    if winds.is_empty() {
        return;
    }

    let direction: Vec2 = winds.iter().map(|wind| wind.direction.normalize()).sum();
    let wind_force =
        Vector::new(direction.x.into(), 0.0, direction.y.into()) * config.wind_strength;

    for player in players {
        if holes.iter().any(|colliding| colliding.contains(&player)) {
            // todo: delay to disable wind while inside hole?
            continue;
        }

        forces
            .get_mut(player)
            .unwrap()
            .apply_force(wind_force);
    }
}

fn despawn_winds(winds: Query<Entity, With<Wind>>, mut commands: Commands) {
    winds.iter().for_each(|e| commands.entity(e).despawn());
}

#[derive(Component, Reflect)]
struct HoleMagnetPowerUp;

fn apply_hole_magnet(
    current_hole: Res<CurrentHole>,
    transforms: Query<&GlobalTransform>,
    players: Query<(Entity, &GlobalTransform), (With<Player>, With<HoleMagnetPowerUp>)>,
    time: Res<Time<Fixed>>,
    config: Res<Configuration>,
    mut forces: Query<Forces>
) {
    let Ok(hole_transform) = transforms.get(current_hole.hole_entity) else {
        return;
    };

    for (player, transform) in players.iter() {
        let vector = hole_transform.translation() - transform.translation();
        let distance = vector.length();

        if distance >= config.hole_magnet_max_distance
            || distance <= config.hole_magnet_min_distance
        {
            continue;
        }

        let force = vector.normalize() * time.delta_secs() * config.hole_magnet_strength;
        forces.get_mut(player).unwrap().apply_force(force.into());
    }
}

fn remove_hole_magnet(
    players: Query<
        Entity,
        (
            With<Player>,
            With<HoleMagnetPowerUp>,
            Changed<LastPlayerPosition>,
        ),
    >,
    mut commands: Commands,
) {
    players.iter().for_each(|player| {
        commands.entity(player).remove::<HoleMagnetPowerUp>();
    });
}

#[derive(Component, Reflect)]
pub(crate) struct StickyBall;

/// Whether the player was awake at the start of the current physics step.
///
/// Bodies are woken up after the solver, so in the step where a sleeping ball is woken up (e.g. by
/// a shot), its contacts still contain data from before it fell asleep.
#[derive(Component, Reflect, Default)]
struct AwakeAtStepStart(bool);

fn record_awake_at_step_start(mut players: Query<(&mut AwakeAtStepStart, Has<Sleeping>)>) {
    for (mut awake, sleeping) in &mut players {
        awake.0 = !sleeping;
    }
}

fn apply_sticky_effects(
    mut players: Query<
        (
            Entity,
            &Player,
            &AwakeAtStepStart,
            Has<StickyBall>,
            &mut LinearVelocity,
            &mut AngularVelocity,
        ),
        Without<Sleeping>,
    >,
    walls: Query<Option<&CourseEffect>, With<HoleWalls>>,
    collisions: Collisions,
    mut commands: Commands,
) {
    for (player_entity, player, awake, sticky_ball, mut linear, mut angular) in &mut players {
        // Contacts are outdated in the step where the ball wakes up.
        if player.can_move || !awake.0 {
            continue;
        }

        let hit_walls = collisions.collisions_with(player_entity).find(|pair| {
            let other_entity = match pair.collider1 == player_entity {
                true => pair.collider2,
                false => pair.collider1,
            };

            let Ok(effect) = walls.get(other_entity) else {
                return false;
            };

            if !sticky_ball && effect != Some(&CourseEffect::StickyWalls) {
                return false;
            }

            // Only stick when the ball hits the wall. Otherwise a ball that was stuck to a wall
            // gets stuck again right after the next shot, when it is moving away from or along it.
            // Contact points also include speculative contacts for walls the ball might reach, so
            // also require that the wall actually pushed the ball back during this step.
            pair.manifolds
                .iter()
                .flat_map(|manifold| manifold.points.iter())
                .any(|point| {
                    point.normal_speed < -STICKY_MIN_APPROACH_SPEED && point.normal_impulse > 0.0
                })
        });

        let Some(hit_walls) = hit_walls else {
            continue;
        };

        info!(
            "Applying sticky effect for player {:?}, walls {:?}",
            player_entity, hit_walls
        );

        commands.entity(player_entity).insert(Sleeping);
        linear.0 = Vector::ZERO;
        angular.0 = Vector::ZERO;
    }
}

/// Minimum speed (m/s) towards a wall at which a sticky effect is applied.
const STICKY_MIN_APPROACH_SPEED: Scalar = 0.01;

fn remove_sticky_ball(players: Query<Entity, With<Player>>, mut commands: Commands) {
    players.iter().for_each(|entity| {
        commands.entity(entity).remove::<StickyBall>();
    });
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            PhysicsConfigPlugin, get_ball_physics_bundle,
            course::{CurrentHole, Hole},
            move_player, player_can_move, reset_can_move,
        },
        bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy},
        minigolf::lobby::PlayerId,
        std::{f32::consts::PI, time::Duration},
    };

    #[test]
    fn ball_sticks_to_wall() {
        let mut app = app();
        spawn_walls(&mut app, true);
        let ball = spawn_ball(&mut app, 0.0, 0.0);

        shoot(&mut app, ball, Vec2::X);
        step(&mut app, 200);

        assert!(can_move(&app, ball));
        let position = position(&app, ball);
        assert!(
            (position.x - AGAINST_WALL_X).abs() < 0.002,
            "ball should rest against the wall, but is at {position}"
        );
    }

    #[test]
    fn fast_ball_sticks_at_wall() {
        let mut app = app();
        spawn_walls(&mut app, true);
        let ball = spawn_ball(&mut app, 0.0, 0.0);

        shoot(&mut app, ball, Vec2::X * 10.0);
        step(&mut app, 200);

        assert!(can_move(&app, ball));
        let position = position(&app, ball);
        assert!(
            (position.x - AGAINST_WALL_X).abs() < 0.002,
            "ball should rest against the wall, but is at {position}"
        );
    }

    #[test]
    fn ball_bounces_off_normal_wall() {
        let mut app = app();
        spawn_walls(&mut app, false);
        let ball = spawn_ball(&mut app, 0.0, 0.0);

        shoot(&mut app, ball, Vec2::X);
        step(&mut app, 200);

        let position = position(&app, ball);
        assert!(
            position.x < AGAINST_WALL_X - 0.05,
            "ball should bounce back from the wall, but is at {position}"
        );
    }

    #[test]
    fn ball_can_leave_wall_it_is_stuck_to() {
        let mut app = app();
        spawn_walls(&mut app, true);
        let ball = spawn_ball(&mut app, 0.0, 0.0);

        shoot(&mut app, ball, Vec2::X);
        step(&mut app, 200);

        shoot(&mut app, ball, Vec2::NEG_X);
        step(&mut app, 60);

        let position = position(&app, ball);
        assert!(
            position.x < AGAINST_WALL_X - 0.2,
            "ball should move away from the wall, but is at {position}"
        );
    }

    #[test]
    fn ball_rolls_along_wall_and_sticks_to_perpendicular_wall() {
        let mut app = app();
        spawn_walls(&mut app, true);
        // Touching the wall at z = -0.075.
        let ball = spawn_ball(&mut app, 0.0, (-0.075 + BALL_RADIUS) as f32);

        shoot(&mut app, ball, Vec2::X);
        step(&mut app, 200);

        assert!(can_move(&app, ball));
        let position = position(&app, ball);
        assert!(
            (position.x - AGAINST_WALL_X).abs() < 0.002,
            "ball should roll along the wall and stick to the perpendicular wall, but is at {position}"
        );
    }

    #[test]
    fn bumper_is_placed_on_floor_relative_to_hole() {
        let mut app = app();
        let hole = spawn_hole(&mut app);
        step(&mut app, 1);

        let transform = place_bumper(&mut app, Vec3::new(2.3, 0.0, 0.1)).unwrap();

        let world = app
            .world()
            .get::<GlobalTransform>(hole)
            .unwrap()
            .transform_point(transform.translation);
        let expected = Vec3::new(2.3, (BUMPER_HEIGHT / 2.0) as f32, 0.1);
        assert!(
            world.distance(expected) < 0.001,
            "bumper should be at {expected}, but is at {world}"
        );
    }

    #[test]
    fn bumper_is_not_placed_outside_current_hole() {
        let mut app = app();
        spawn_hole(&mut app);
        step(&mut app, 1);

        assert_eq!(place_bumper(&mut app, Vec3::new(0.3, 0.0, 0.1)), None);
    }

    #[test]
    fn bumper_is_not_placed_on_ball() {
        let mut app = app();
        spawn_hole(&mut app);
        spawn_ball(&mut app, 2.3, 0.1);
        step(&mut app, 1);

        assert_eq!(place_bumper(&mut app, Vec3::new(2.32, 0.0, 0.1)), None);
    }

    /// Spawns the current hole with a floor at y = 0, offset and rotated like the holes of a
    /// course.
    fn spawn_hole(app: &mut App) -> Entity {
        let hole = app
            .world_mut()
            .spawn((
                Name::new("Hole"),
                RigidBody::Static,
                Collider::cuboid(1.0, 0.1, 1.0),
                Transform::from_xyz(2.0, -0.05, 0.0).with_rotation(Quat::from_rotation_y(PI)),
            ))
            .id();

        app.world_mut().resource_mut::<CurrentHole>().hole_entity = hole;
        hole
    }

    fn place_bumper(app: &mut App, translation: Vec3) -> Option<Transform> {
        app.world_mut()
            .run_system_once(move |placement: BumperPlacement| placement.place(translation))
            .unwrap()
    }

    const BALL_RADIUS: Scalar = 0.021336;
    /// Impulse that gives the ball a speed of about 1 m/s.
    const SHOT: f32 = 0.046;

    /// X coordinate of the ball's center when resting against the wall at x = 0.5.
    const AGAINST_WALL_X: Scalar = 0.5 - 0.025 - BALL_RADIUS;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            bevy::scene::ScenePlugin,
            bevy::diagnostic::DiagnosticsPlugin,
            PhysicsPlugins::default(),
            PhysicsConfigPlugin,
        ));
        // Normally registered by the diagnostics UI.
        app.init_resource::<avian3d::collider_tree::ColliderTreeDiagnostics>();
        app.init_resource::<avian3d::spatial_query::SpatialQueryDiagnostics>();
        app.init_resource::<avian3d::collision::CollisionDiagnostics>();
        app.init_resource::<avian3d::dynamics::solver::SolverDiagnostics>();

        // Run exactly one physics step per update.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 128.0,
        )));

        app.register_required_components::<Player, AwakeAtStepStart>();
        app.add_message::<ValidPlayerInput>();
        app.insert_resource(CurrentHole {
            hole: Hole {
                start_position: Vec3::ZERO,
            },
            hole_entity: Entity::PLACEHOLDER,
            players: vec![],
        });

        app.add_systems(
            PhysicsSchedule,
            (
                record_awake_at_step_start.in_set(PhysicsStepSystems::First),
                apply_sticky_effects.in_set(PhysicsStepSystems::Finalize),
            ),
        );
        app.add_systems(FixedUpdate, player_can_move);
        app.add_systems(Update, (move_player, reset_can_move));

        app.world_mut().spawn((
            Name::new("Floor"),
            RigidBody::Static,
            Collider::cuboid(2.0, 0.1, 2.0),
            Transform::from_xyz(0.0, -0.05, 0.0),
        ));

        app
    }

    /// Spawns walls with a wall at x = 0.5 and a wall at z = -0.075, as a single collider.
    fn spawn_walls(app: &mut App, sticky: bool) {
        let mut walls = app.world_mut().spawn((
            Name::new("Walls"),
            HoleWalls {
                hole_entity: Entity::PLACEHOLDER,
            },
            walls_collider(),
            Transform::default(),
        ));

        if sticky {
            walls.insert(CourseEffect::StickyWalls);
        }
    }

    /// Trimesh collider like the one created from the walls mesh of a hole.
    fn walls_collider() -> Collider {
        let wall =
            |center: Vec3, size: Vec3| Mesh::from(Cuboid::from_size(size)).translated_by(center);

        let mut mesh = wall(Vec3::new(0.5, 0.0, 0.0), Vec3::new(0.05, 0.3, 2.0));
        mesh.merge(&wall(Vec3::new(0.0, 0.0, -0.1), Vec3::new(2.0, 0.3, 0.05)))
            .unwrap();

        Collider::trimesh_from_mesh_with_config(&mesh, TrimeshFlags::all()).unwrap()
    }

    fn spawn_ball(app: &mut App, x: f32, z: f32) -> Entity {
        app.world_mut()
            .spawn((
                Player {
                    id: PlayerId::new(),
                    can_move: true,
                },
                LastPlayerPosition {
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                },
                Transform::from_xyz(x, BALL_RADIUS as f32, z),
                get_ball_physics_bundle(),
            ))
            .id()
    }

    fn shoot(app: &mut App, ball: Entity, direction: Vec2) {
        assert!(
            can_move(app, ball),
            "ball must be able to move before shooting"
        );

        app.world_mut().write_message(ValidPlayerInput {
            player: ball,
            input: PlayerInput::Move(direction * SHOT),
        });
    }

    fn step(app: &mut App, steps: usize) {
        for _ in 0..steps {
            app.update();
        }
    }

    fn position(app: &App, ball: Entity) -> Vector {
        app.world().get::<Position>(ball).unwrap().0
    }

    fn can_move(app: &App, ball: Entity) -> bool {
        app.world().get::<Player>(ball).unwrap().can_move
    }
}
