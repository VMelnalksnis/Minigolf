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
            .register_type::<HoleBoundingBox>()
            .register_type::<PlayerScore>();

        app.add_event::<CourseCompleted>();

        app.add_observer(on_hole_added);

        app.add_systems(OnEnter(ServerState::WaitingForPlayers), setup_level);
        app.add_systems(OnExit(ServerState::Playing), despawn_level);

        app.configure_sets(Update, PlayingSet.run_if(in_state(ServerState::Playing)))
            .configure_sets(
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

#[derive(Event)]
pub(crate) struct CourseCompleted;

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

#[derive(Component, Reflect, Default, Debug)]
pub(crate) struct PlayerScore {
    score: u32,
}

#[derive(Resource)]
pub(crate) struct CurrentHole {
    pub(crate) hole: Hole,
    entity: Entity,
    pub(crate) players: Vec<Player>,
}

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
struct PlayingSet;

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

    let hole1_path = "Level2.glb#Mesh0/Primitive0";

    for index in 1..3 {
        let level_mesh_handle: Handle<Mesh> = server.load(hole1_path);

        let x_offset = (index - 1) as f32 * 10.0;
        let start_position = Vec3::new(x_offset, 2.0, 0.0);

        let hole = commands
            .spawn((
                Name::new(format!("Hole {}", index)),
                LevelMesh::from_path(hole1_path),
                Hole { start_position },
                Replicated,
                Transform::from_xyz(x_offset, 0.0, 0.0),
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
                Name::new(format!("Hole {} sensor", index)),
                Transform::from_xyz(1.6, -0.11, 0.0),
                Sensor,
                HoleSensor::new(hole),
                RigidBody::Static,
                CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
                Collider::cuboid(0.2, 0.19, 1.0),
                CollidingEntities::default(),
            ))
            .set_parent(hole);

        commands
            .spawn((
                Name::new(format!("Hole {} bounding box", index)),
                Transform::from_xyz(1.0, 0.0, 0.0),
                Sensor,
                HoleBoundingBox::new(hole),
                RigidBody::Static,
                CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
                Collider::cuboid(3.1, 2.1, 2.1),
                CollidingEntities::default(),
            ))
            .set_parent(hole);
    }
}

fn on_hole_added(
    trigger: Trigger<OnAdd, Hole>,
    mut course: Query<&mut Course>,
    hole: Query<&Hole>,
    mut commands: Commands,
) {
    let hole_entity = trigger.entity();
    let mut course = course.single_mut();
    course.holes.push(hole_entity);

    if let &[_] = course.holes.as_slice() {
        let hole = hole.get(hole_entity).unwrap();
        commands.insert_resource::<CurrentHole>(CurrentHole {
            hole: *hole,
            entity: hole_entity,
            players: vec![],
        });
    }
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

fn handle_hole_bounding_box(
    bounds: Query<(Entity, &HoleBoundingBox, &CollidingEntities), Changed<CollidingEntities>>,
    players: Query<(Entity, &Player)>,
    mut transforms: Query<
        (
            &mut Transform,
            &mut LinearVelocity,
            &mut AngularVelocity,
            &crate::server::LastPlayerPosition,
        ),
        With<Player>,
    >,
    current_hole: Res<CurrentHole>,
) {
    for (bounds_entity, bounding_box, colliding_entities) in bounds.iter() {
        if current_hole.entity != bounding_box.hole {
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

                linear.0 = Vec3::ZERO;
                angular.0 = Vec3::ZERO;

                info!("Last position: {last:?}");
                transform.translation = last.position;
                transform.rotation = last.rotation;
                info!("{transform:?}");
            }
        }
    }
}

fn current_hole_modified(
    mut current_hole: ResMut<CurrentHole>,
    mut players: Query<&mut Transform, With<Player>>,
    course: Query<&Course>,
    holes: Query<&Hole>,
    mut writer: EventWriter<CourseCompleted>,
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

    let _ = current_hole.players.drain(..).collect::<Vec<_>>();
    let course = course.single();
    info!(
        "Course {:?}, current hole {:?}",
        course, current_hole.entity
    );

    let remaining_holes = course
        .holes
        .iter()
        .skip_while(|h| !current_hole.entity.eq(*h))
        .skip(1)
        .collect::<Vec<_>>();

    info!("Remaining holes {:?}", remaining_holes);

    let Some(next_hole) = remaining_holes.first() else {
        writer.send(CourseCompleted);
        return;
    };

    let c = *holes.get(**next_hole).unwrap();
    current_hole.entity = **next_hole;
    current_hole.hole = c;

    for mut transform in &mut players {
        transform.translation = c.start_position;
    }
}

fn despawn_level(scenes: Query<Entity, With<SceneRoot>>, mut commands: Commands) {
    for scene in scenes.iter() {
        commands.entity(scene).despawn();
    }
}
