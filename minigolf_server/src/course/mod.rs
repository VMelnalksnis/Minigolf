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
    minigolf::{LevelMesh, Player, PlayerInput, PlayerScore, PowerUp, PowerUpType},
    rand::Rng,
    std::f32::consts::PI,
};

pub(crate) struct CoursePlugin;

impl Plugin for CoursePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PowerUpPlugin);

        app.register_type::<Course>()
            .register_type::<Hole>()
            .register_type::<HoleSensor>()
            .register_type::<HoleBoundingBox>()
            .register_type::<HoleWalls>();

        app.register_type::<Bumper>();
        app.register_type::<JumpPad>();

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

/// Component for identifying jump pad entities.
#[derive(Component, Reflect, Debug)]
pub(crate) struct JumpPad;

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

    let bumper_path = "Entities.glb#Mesh1/Primitive0";

    let floor_1_path = "courses/0002.glb#Mesh0/Primitive0";
    let floor_1_handle: Handle<Mesh> = server.load(floor_1_path);
    let walls_1_path = "courses/0002.glb#Mesh1/Primitive0";
    let walls_1_handle: Handle<Mesh> = server.load(walls_1_path);

    let hole_1 = commands
        .spawn(hole_bundle(
            "Hole 1".to_string(),
            course,
            Vec3::new(0.0, 0.5, 0.0),
            Transform::from_xyz(0.0, 0.0, 0.0),
            floor_1_path,
            floor_1_handle,
        ))
        .id();

    commands.spawn(hole_walls_bundle(
        "Hole 1 walls".to_string(),
        hole_1,
        walls_1_path,
        walls_1_handle,
    ));

    commands.spawn(hole_bounding_box_bundle(
        "Hole 1 bounding box".to_string(),
        hole_1,
        Transform::from_xyz(0.6, 0.2, 0.0),
        Collider::cuboid(2.0, 4.0, 0.8),
    ));

    commands.spawn(hole_sensor_bundle(
        "Hole 1 sensor".to_string(),
        hole_1,
        Transform::from_xyz(1.2, -0.05, 0.0),
    ));

    commands.spawn(power_up_bundle(
        hole_1,
        Transform::from_xyz(0.8, 0.025, 0.0),
    ));

    let floor_2_path = "courses/0002.glb#Mesh2/Primitive0";
    let floor_2_handle: Handle<Mesh> = server.load(floor_2_path);
    let walls_2_path = "courses/0002.glb#Mesh3/Primitive0";
    let walls_2_handle: Handle<Mesh> = server.load(walls_2_path);

    let hole_02 = commands
        .spawn(hole_bundle(
            "Hole 2".to_string(),
            course,
            Vec3::new(2.0, 0.5, 0.0),
            Transform::from_xyz(2.0, 0.0, 0.0),
            floor_2_path,
            floor_2_handle,
        ))
        .id();

    commands.spawn(hole_walls_bundle(
        "Hole 2 walls".to_string(),
        hole_02,
        walls_2_path,
        walls_2_handle,
    ));

    commands.spawn(hole_bounding_box_bundle(
        "Hole 2 bounding box".to_string(),
        hole_02,
        Transform::from_xyz(1.0, 0.2, -0.2),
        Collider::cuboid(3.0, 4.0, 1.2),
    ));

    commands.spawn(hole_sensor_bundle(
        "Hole 2 sensor".to_string(),
        hole_02,
        Transform::from_xyz(2.0, -0.05, 0.0),
    ));

    commands.spawn(jump_pad_bundle(
        hole_02,
        Transform::from_xyz(0.8, 0.05, 0.0),
    ));

    let floor_3_path = "courses/0002.glb#Mesh4/Primitive0";
    let floor_3_handle: Handle<Mesh> = server.load(floor_3_path);
    let walls_3_path = "courses/0002.glb#Mesh5/Primitive0";
    let walls_3_handle: Handle<Mesh> = server.load(walls_3_path);

    let hole_03 = commands
        .spawn(hole_bundle(
            "Hole 3".to_string(),
            course,
            Vec3::new(4.0, 0.5, 0.8),
            Transform::from_xyz(4.0, 0.0, 0.8).with_rotation(Quat::from_euler(EulerRot::XYZ, 0.0, PI, 0.0)),
            floor_3_path,
            floor_3_handle,
        ))
        .id();

    commands.spawn(hole_walls_bundle(
        "Hole 3 walls".to_string(),
        hole_03,
        walls_3_path,
        walls_3_handle,
    ));

    commands.spawn(hole_bounding_box_bundle(
        "Hole 3 bounding box".to_string(),
        hole_03,
        Transform::from_xyz(0.6, 0.2, 0.2),
        Collider::cuboid(2.0, 4.0, 1.2),
    ));

    commands.spawn(hole_sensor_bundle(
        "Hole 3 sensor".to_string(),
        hole_03,
        Transform::from_xyz(1.2, -0.05, 0.0),
    ));

    commands.spawn(bumper_bundle(
        hole_03,
        Transform::from_xyz(0.8, 0.025, -0.4),
        bumper_path.to_string(),
    ));

    commands.spawn((
        Name::new("Bumper collision observer"),
        StateScoped(ServerState::Playing),
        Observer::new(on_bumper_collision),
    ));

    commands.spawn((
        Name::new("Jump pad collision observer"),
        StateScoped(ServerState::Playing),
        Observer::new(on_jump_pad_collision),
    ));
}

fn hole_bundle(
    name: String,
    course_entity: Entity,
    start_position: Vec3,
    transform: Transform,
    asset_path: &str,
    mesh: Handle<Mesh>,
) -> impl Bundle {
    (
        Name::new(name),
        Hole { start_position },
        transform,
        Replicated,
        Mesh3d(mesh),
        LevelMesh::from_path(asset_path),
        RigidBody::Static,
        ColliderConstructor::TrimeshFromMeshWithConfig(TrimeshFlags::all()),
        CollisionLayers::new(GameLayer::Default, [GameLayer::Default, GameLayer::Player]),
        Friction::new(0.9),
        Restitution::new(0.1),
        ChildOf(course_entity),
    )
}

fn hole_walls_bundle(
    name: String,
    hole_entity: Entity,
    asset_path: &str,
    mesh: Handle<Mesh>,
) -> impl Bundle {
    (
        Name::new(name),
        Transform::IDENTITY,
        HoleWalls { hole_entity },
        Replicated,
        Mesh3d(mesh),
        LevelMesh::from_path(asset_path),
        RigidBody::Static,
        ColliderConstructor::TrimeshFromMeshWithConfig(TrimeshFlags::all()),
        CollisionLayers::new(GameLayer::Default, [GameLayer::Default, GameLayer::Player]),
        Friction::new(0.8).with_combine_rule(CoefficientCombine::Multiply),
        Restitution::new(0.9).with_combine_rule(CoefficientCombine::Max),
        ChildOf(hole_entity),
    )
}

fn hole_bounding_box_bundle(
    name: String,
    hole_entity: Entity,
    transform: Transform,
    collider: Collider,
) -> impl Bundle {
    (
        Name::new(name),
        transform,
        Sensor,
        HoleBoundingBox::new(hole_entity),
        RigidBody::Static,
        CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
        collider,
        CollidingEntities::default(),
        ChildOf(hole_entity),
    )
}

fn hole_sensor_bundle(name: String, hole_entity: Entity, transform: Transform) -> impl Bundle {
    (
        Name::new(name),
        transform,
        Sensor,
        HoleSensor::new(hole_entity),
        RigidBody::Static,
        CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
        Collider::cuboid(0.2, 0.09, 0.2),
        CollidingEntities::default(),
        ChildOf(hole_entity),
    )
}

fn power_up_bundle(hole_entity: Entity, transform: Transform) -> impl Bundle {
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
        ChildOf(hole_entity),
    )
}

fn bumper_bundle(hole_entity: Entity, transform: Transform, asset: String) -> impl Bundle {
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
        ChildOf(hole_entity),
    )
}

fn jump_pad_bundle(hole_entity: Entity, transform: Transform) -> impl Bundle {
    (
        Name::new("Jump pad"),
        JumpPad,
        transform,
        RigidBody::Static,
        Collider::cylinder(0.085344, 0.05),
        CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
        Sensor,
        Replicated,
        CollisionEventsEnabled,
        ChildOf(hole_entity),
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

    // todo: probably should handle collisions from above differently
    let direction = (player_position.0 - bumper_position.0).normalize();

    info!(
        "Applying bumper effect to player {:?} in direction {:?}",
        other_entity, direction
    );

    commands
        .entity(other_entity)
        .insert(ExternalImpulse::new(direction * BUMPER_STRENGTH).with_persistence(false));
}

const JUMP_PAD_STRENGTH: f64 = 0.2;

fn on_jump_pad_collision(
    trigger: Trigger<OnCollisionStart>,
    jump_pads: Query<(), With<JumpPad>>,
    players: Query<(), With<Player>>,
    mut commands: Commands,
) {
    let jump_pad_entity = trigger.target();
    let Ok(_) = jump_pads.get(jump_pad_entity) else {
        return;
    };

    let other_entity = trigger.0;
    let Ok(_) = players.get(other_entity) else {
        return;
    };

    let direction = DVec3::Y;

    info!(
        "Applying jump pad effect to player {:?} in direction {:?}",
        other_entity, direction
    );

    commands
        .entity(other_entity)
        .insert(ExternalImpulse::new(direction * JUMP_PAD_STRENGTH).with_persistence(false));
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
