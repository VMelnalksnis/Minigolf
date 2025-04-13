use {
    crate::{
        ServerState,
        server::{GameLayer, ValidPlayerInput},
    },
    avian3d::prelude::*,
    bevy::{app::App, math::Vec3, prelude::*},
    bevy_replicon::prelude::Replicated,
    minigolf::{LevelMesh, Player},
};

pub(crate) struct CoursePlugin;

impl Plugin for CoursePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Course>()
            .register_type::<Hole>()
            .register_type::<HoleSensor>()
            .register_type::<PlayerScore>();

        app.add_observer(on_hole_added);

        app.add_systems(OnEnter(ServerState::WaitingForPlayers), setup_level);
        app.add_systems(OnExit(ServerState::Playing), despawn_level);

        app.add_systems(Update, (increment_score, log_score_changes));
        app.add_systems(FixedUpdate, handle_hole_sensors);
    }
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

#[derive(Component, Reflect, Debug)]
pub(crate) struct Hole {
    start_position: Vec3,
}
impl Hole {
    pub(crate) fn new() -> Self {
        Hole {
            start_position: Vec3::ZERO,
        }
    }
}

#[derive(Component, Reflect, Debug)]
pub(crate) struct HoleSensor {
    hole: Entity,
}

impl HoleSensor {
    pub(crate) fn new(hole: Entity) -> Self {
        HoleSensor { hole }
    }
}

#[derive(Component, Reflect, Default, Debug)]
pub(crate) struct PlayerScore {
    score: u32,
}

fn setup_level(mut commands: Commands, server: Res<AssetServer>) {
    let scene = commands
        .spawn((Name::new("Scene"), SceneRoot::default()))
        .id();

    let course = commands
        .spawn((
            Name::new("Course"),
            Course::new(),
            Transform::default(),
            Visibility::default(),
        ))
        .set_parent(scene)
        .id();

    let hole1_path = "Level1.glb#Mesh0/Primitive0";
    let level_mesh_handle: Handle<Mesh> = server.load(hole1_path);

    let hole_1 = commands
        .spawn((
            Name::new("Hole 1"),
            LevelMesh::from_path(hole1_path),
            Hole::new(),
            Replicated,
            Transform::from_xyz(4.0, -1.0, 0.0).with_scale(Vec3::new(5.0, 1.0, 1.0)),
            RigidBody::Static,
            ColliderConstructor::TrimeshFromMeshWithConfig(TrimeshFlags::all()),
            Mesh3d(level_mesh_handle),
            CollisionLayers::new(GameLayer::Default, [GameLayer::Default, GameLayer::Player]),
            Friction::new(0.8).with_combine_rule(CoefficientCombine::Multiply),
            Restitution::new(0.7).with_combine_rule(CoefficientCombine::Multiply),
        ))
        .set_parent(course)
        .id();

    commands
        .spawn((
            Name::new("Hole 1 sensor"),
            Transform::from_xyz(0.8, 0.9, 0.0),
            Sensor,
            HoleSensor::new(hole_1),
            RigidBody::Static,
            CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
            Collider::cuboid(0.2, 0.19, 1.0),
            CollidingEntities::default(),
        ))
        .set_parent(hole_1);
}

fn on_hole_added(trigger: Trigger<OnAdd, Hole>, mut course: Query<&mut Course>) {
    let hole_entity = trigger.entity();
    let mut course = course.single_mut();
    course.holes.push(hole_entity);
}

fn increment_score(mut reader: EventReader<ValidPlayerInput>, mut scores: Query<&mut PlayerScore>) {
    for input in reader.read() {
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
) {
    for (hole, hole_collisions) in holes.iter() {
        for (player_entity, player) in players.iter() {
            if hole_collisions.contains(&player_entity) {
                info!("Player {:?} collided with hole {:?}", player, hole)
            } else {
                info!("Player {:?} left hole {:?}", player, hole)
            }
        }
    }
}

fn despawn_level(scenes: Query<Entity, With<SceneRoot>>, mut commands: Commands) {
    for scene in scenes.iter() {
        commands.entity(scene).despawn();
    }
}
