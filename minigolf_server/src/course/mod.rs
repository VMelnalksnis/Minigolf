mod power_ups;

use {
    crate::{
        ServerState,
        course::power_ups::PowerUpPlugin,
        server::{GameLayer, LastPlayerPosition, ValidPlayerInput},
    },
    avian3d::prelude::*,
    bevy::{app::App, math::DVec3, prelude::*},
    bevy_replicon::prelude::*,
    minigolf::{LevelMesh, Player, PlayerInput, PowerUp, PowerUpType},
    rand::Rng,
};

pub(crate) struct CoursePlugin;

impl Plugin for CoursePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PowerUpPlugin);

        app.register_type::<Course>()
            .register_type::<Hole>()
            .register_type::<HoleSensor>()
            .register_type::<HoleBoundingBox>()
            .register_type::<HoleWalls>()
            .register_type::<PlayerScore>();

        app.register_type::<Bumper>();

        app.add_observer(on_hole_added);

        app.add_systems(OnEnter(ServerState::WaitingForPlayers), setup_course);

        app.add_systems(OnEnter(ServerState::Playing), setup_playing);
        app.add_systems(OnExit(ServerState::Playing), despawn_level);

        app.configure_sets(Update, PlayingSet.run_if(in_state(ServerState::Playing)));
        app.configure_sets(
            FixedUpdate,
            PlayingSet.run_if(in_state(ServerState::Playing)),
        );

        app.add_systems(
            Update,
            (increment_score, log_score_changes).in_set(PlayingSet),
        );

        app.add_systems(
            FixedUpdate,
            (
                handle_hole_sensors,
                handle_hole_bounding_box,
                current_hole_modified,
            )
                .in_set(PlayingSet),
        );
    }
}

#[derive(Event, Reflect, Debug)]
pub(crate) struct HoleCompleted;

#[derive(Event, Reflect, Debug)]
pub(crate) struct CourseCompleted;

fn setup_playing(mut commands: Commands) {
    commands.spawn((
        Name::new("Course Completed observer"),
        Observer::new(on_course_completed),
        StateScoped(ServerState::Playing),
    ));

    commands.spawn((
        Name::new("Hole Completed observer"),
        Observer::new(on_hole_completed),
        StateScoped(ServerState::Playing),
    ));
}

fn on_course_completed(
    _trigger: Trigger<CourseCompleted>,
    mut state: ResMut<NextState<ServerState>>,
) {
    state.set(ServerState::WaitingForGame);
}

#[derive(Component, Reflect, Debug)]
pub(crate) struct Course {
    holes: Vec<Entity>,
}

impl Course {
    pub(crate) fn new() -> Self {
        Course { holes: vec![] }
    }
}

#[derive(Component, Reflect, Copy, Clone, Debug)]
pub(crate) struct Hole {
    pub(crate) start_position: Vec3,
}

#[derive(Component, Reflect, Copy, Clone, Debug)]
pub(crate) struct HoleSensor {
    hole: Entity,
}

impl HoleSensor {
    pub(crate) fn new(hole: Entity) -> Self {
        HoleSensor { hole }
    }
}

#[derive(Component, Reflect, Copy, Clone, Debug)]
pub(crate) struct HoleBoundingBox {
    hole: Entity,
}

impl HoleBoundingBox {
    pub(crate) fn new(hole: Entity) -> Self {
        HoleBoundingBox { hole }
    }
}

#[derive(Component, Reflect, Debug)]
struct HoleWalls {
    hole_entity: Entity,
}

#[derive(Component, Reflect, Default, Debug)]
pub(crate) struct PlayerScore {
    score: u32,
}

#[derive(Resource)]
pub(crate) struct CurrentHole {
    pub(crate) hole: Hole,
    hole_entity: Entity,
    pub(crate) players: Vec<Player>,
}

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct PlayingSet;

/// Component for identifying bumper entities.
#[derive(Component, Reflect, Debug)]
pub(crate) struct Bumper;

fn setup_course(mut commands: Commands, server: Res<AssetServer>) {
    let scene = commands
        .spawn((Name::new("Scene"), SceneRoot::default(), Replicated))
        .id();

    let course = commands
        .spawn((
            Name::new("Course"),
            Course::new(),
            Transform::default(),
            Visibility::default(),
            Replicated,
        ))
        .insert(ChildOf(scene))
        .id();

    let floor_path = "Course1.glb#Mesh4/Primitive0";
    let floor_handle: Handle<Mesh> = server.load(floor_path);

    let walls_path = "Course1.glb#Mesh3/Primitive0";
    let walls_handle: Handle<Mesh> = server.load(walls_path);

    let bumper_path = "Entities.glb#Mesh1/Primitive0";

    for index in 0..2 {
        let offset = 2.4;
        let x_offset = offset + index as f32 * 4.0;

        let hole = commands
            .spawn((
                Name::new(format!("Hole {index}")),
                Hole {
                    start_position: Vec3::new(x_offset, 0.5, 0.0),
                },
                Transform::from_xyz(x_offset + offset, 0.0, 0.0),
                Replicated,
                Mesh3d(floor_handle.clone()),
                LevelMesh::from_path(floor_path),
                RigidBody::Static,
                ColliderConstructor::TrimeshFromMeshWithConfig(TrimeshFlags::all()),
                CollisionLayers::new(GameLayer::Default, [GameLayer::Default, GameLayer::Player]),
                Friction::new(0.9),
                Restitution::new(0.1),
            ))
            .insert(ChildOf(course))
            .id();

        commands
            .spawn((
                Name::new(format!("Hole {index} walls")),
                Transform::from_xyz(-offset, 0.0, 0.0),
                HoleWalls { hole_entity: hole },
                Replicated,
                Mesh3d(walls_handle.clone()),
                LevelMesh::from_path(walls_path),
                RigidBody::Static,
                ColliderConstructor::TrimeshFromMeshWithConfig(TrimeshFlags::all()),
                CollisionLayers::new(GameLayer::Default, [GameLayer::Default, GameLayer::Player]),
                Friction::new(0.8).with_combine_rule(CoefficientCombine::Multiply),
                Restitution::new(0.9).with_combine_rule(CoefficientCombine::Max),
            ))
            .insert(ChildOf(hole));

        commands
            .spawn((
                Name::new(format!("Hole {index} bounding box")),
                Transform::from_xyz(-offset + 1.0, 0.2, 0.0),
                Sensor,
                HoleBoundingBox::new(hole),
                RigidBody::Static,
                CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
                Collider::cuboid(4.0, 2.0, 3.0),
                CollidingEntities::default(),
            ))
            .insert(ChildOf(hole));

        commands
            .spawn((
                Name::new(format!("Hole {index} sensor")),
                Transform::from_xyz(-offset + 2.4, -0.05, 0.0),
                Sensor,
                HoleSensor::new(hole),
                RigidBody::Static,
                CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
                Collider::cuboid(0.2, 0.09, 0.2),
                CollidingEntities::default(),
            ))
            .insert(ChildOf(hole));

        commands
            .spawn(power_up_bundle(Transform::from_xyz(
                -offset + 1.2,
                0.15,
                0.0,
            )))
            .insert(ChildOf(hole));

        commands
            .spawn(power_up_bundle(Transform::from_xyz(
                -offset + 1.2,
                0.05,
                0.8,
            )))
            .insert(ChildOf(hole));

        commands
            .spawn(power_up_bundle(Transform::from_xyz(
                -offset + 0.0,
                0.05,
                0.8,
            )))
            .insert(ChildOf(hole));

        commands
            .spawn(bumper_bundle(
                Transform::from_xyz(-offset + 0.0, 0.025, -0.8),
                bumper_path.to_string(),
            ))
            .insert(ChildOf(hole));
    }

    commands.spawn((
        Name::new("Bumper collision observer"),
        StateScoped(ServerState::Playing),
        Observer::new(on_bumper_collision),
    ));
}

fn power_up_bundle(transform: Transform) -> impl Bundle {
    (
        Name::new("Power up"),
        transform,
        Sensor,
        RigidBody::Static,
        CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
        Collider::sphere(0.1),
        CollidingEntities::default(),
        PowerUp::from(rand::rng().random::<PowerUpType>()),
        Replicated,
    )
}

fn bumper_bundle(transform: Transform, asset: String) -> impl Bundle {
    (
        Name::new("Bumper"),
        Bumper,
        transform,
        RigidBody::Static,
        Collider::cylinder(0.042672, 0.05),
        CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
        Replicated,
        LevelMesh { asset },
        CollisionEventsEnabled,
    )
}

const BUMPER_STRENGTH: f64 = 0.1;

fn on_bumper_collision(
    trigger: Trigger<OnCollisionStart>,
    bumpers: Query<&Position, With<Bumper>>,
    players: Query<&Position, With<Player>>,
    mut commands: Commands,
) {
    let bumper_entity = trigger.target();
    let Ok(bumper_position) = bumpers.get(bumper_entity) else {
        return;
    };

    let other_entity = trigger.0;
    let Ok(player_position) = players.get(other_entity) else {
        return;
    };

    let direction = (player_position.0 - bumper_position.0).normalize();

    info!(
        "Applying bumper effect to player {:?} in direction {:?}",
        other_entity, direction
    );

    commands
        .entity(other_entity)
        .insert(ExternalImpulse::new(direction * BUMPER_STRENGTH).with_persistence(false));
}

fn on_hole_added(
    trigger: Trigger<OnAdd, Hole>,
    mut course: Query<&mut Course>,
    hole: Query<&Hole>,
    mut commands: Commands,
) {
    let hole_entity = trigger.target();
    let mut course = course.single_mut().unwrap();
    course.holes.push(hole_entity);

    if let &[_] = course.holes.as_slice() {
        let hole = hole.get(hole_entity).unwrap();
        commands.insert_resource::<CurrentHole>(CurrentHole {
            hole: *hole,
            hole_entity,
            players: vec![],
        });
    }
}

fn increment_score(mut reader: EventReader<ValidPlayerInput>, mut scores: Query<&mut PlayerScore>) {
    for input in reader.read() {
        let PlayerInput::Move(_) = input.input else {
            continue;
        };

        let Ok(mut score) = scores.get_mut(input.player) else {
            warn!("Received {:?} without player score component", input);
            continue;
        };

        score.score += 1;
    }
}

fn log_score_changes(scores: Query<(Entity, &PlayerScore), Changed<PlayerScore>>) {
    for (entity, score) in scores.iter() {
        info!(
            "Increased score to {:?} for player {:?}",
            score.score, entity
        );
    }
}

fn handle_hole_sensors(
    holes: Query<(Entity, &CollidingEntities), (With<HoleSensor>, Changed<CollidingEntities>)>,
    players: Query<(Entity, &Player)>,
    mut commands: Commands,
) {
    for (hole, hole_collisions) in holes.iter() {
        for (player_entity, player) in players.iter() {
            if hole_collisions.contains(&player_entity) {
                info!("Player {:?} collided with hole {:?}", player, hole);

                // todo: should this be done somewhere else? and re-enable wind after exiting?
                commands
                    .entity(player_entity)
                    .insert(ExternalForce::default());
            } else {
                info!("Player {:?} left hole {:?}", player, hole);
            }
        }
    }
}

fn handle_hole_bounding_box(
    bounds: Query<(Entity, &HoleBoundingBox, &CollidingEntities), Changed<CollidingEntities>>,
    players: Query<(Entity, &Player)>,
    mut transforms: Query<
        (
            &mut Transform,
            &mut LinearVelocity,
            &mut AngularVelocity,
            &LastPlayerPosition,
        ),
        With<Player>,
    >,
    current_hole: Res<CurrentHole>,
) {
    for (bounds_entity, bounding_box, colliding_entities) in bounds.iter() {
        if current_hole.hole_entity != bounding_box.hole {
            continue;
        }

        for (player_entity, player) in players.iter() {
            if colliding_entities.contains(&player_entity) {
                info!(
                    "Player {:?} entered bounds of hole {:?}",
                    player, bounds_entity
                );
            } else {
                info!(
                    "Player {:?} left bounds of hole {:?}",
                    player, bounds_entity
                );
                let (mut transform, mut linear, mut angular, last) =
                    transforms.get_mut(player_entity).unwrap();

                linear.0 = DVec3::ZERO;
                angular.0 = DVec3::ZERO;

                info!("Last position: {last:?}");
                // todo: ball rolls off the edge when last position set close to it, even though it was stable before respawning
                // might have to calculate some safety margin in order to avoid issues after respawn
                transform.translation = last.position;
                transform.rotation = last.rotation;
            }
        }
    }
}

fn current_hole_modified(
    current_hole: Res<CurrentHole>,
    players: Query<(), With<Player>>,
    mut commands: Commands,
) {
    if !current_hole.is_changed() {
        return;
    }

    info!("Current hole changed");

    let player_count = players.iter().count();
    let completed_player_count = current_hole.players.len();

    if player_count != completed_player_count {
        info!(
            "Player count {:?} does not match completed player count {:?}",
            player_count, completed_player_count
        );

        return;
    }

    commands.trigger(HoleCompleted);
}

fn on_hole_completed(
    _trigger: Trigger<HoleCompleted>,
    mut current_hole: ResMut<CurrentHole>,
    mut players: Query<(&mut LastPlayerPosition, &mut Transform), With<Player>>,
    course: Query<&Course>,
    holes: Query<&Hole>,
    mut commands: Commands,
) {
    let _ = current_hole.players.drain(..).collect::<Vec<_>>();
    let course = course.single().unwrap();
    info!(
        "Course {:?}, current hole {:?}",
        course, current_hole.hole_entity
    );

    let next_hole = course
        .holes
        .iter()
        .skip_while(|h| current_hole.hole_entity != **h)
        .skip(1)
        .map(|h| *h)
        .next();

    let Some(next_hole_entity) = next_hole else {
        commands.trigger(CourseCompleted);
        return;
    };

    let next_hole = holes.get(next_hole_entity).unwrap();
    current_hole.hole_entity = next_hole_entity;
    current_hole.hole = *next_hole;

    players
        .iter_mut()
        .for_each(|(mut last_position, mut transform)| {
            transform.scale = Vec3::splat(1.0);
            transform.translation = next_hole.start_position;

            last_position.position = next_hole.start_position;
            last_position.rotation = Quat::IDENTITY;
        });
}

fn despawn_level(scenes: Query<Entity, With<SceneRoot>>, mut commands: Commands) {
    for scene in scenes.iter() {
        commands.entity(scene).try_despawn(); // todo: does not despawn power ups
    }
}
