mod entities;
pub(crate) mod power_ups;
pub(crate) mod setup;

use {
    crate::{
        Configuration, CourseState, GameLayer, GameState, HoleState, LastPlayerPosition,
        LoadingCourseSystems, PlayingSystems, ServerState, ValidPlayerInput,
        course::{
            entities::CourseEntitiesPlugin, power_ups::PowerUpPlugin, setup::CourseSetupPlugin,
        },
    },
    avian3d::{math::Vector, prelude::*},
    bevy::{app::App, prelude::*},
    minigolf::{CourseDetails, Player, PlayerInput, PlayerScore, PowerUp},
};

pub(crate) struct CoursePlugin;

impl Plugin for CoursePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CourseEntitiesPlugin);
        app.add_plugins(PowerUpPlugin);
        app.add_plugins(CourseSetupPlugin);

        app.register_type::<GameConfig>();

        app.register_type::<Course>();
        app.register_type::<Hole>();
        app.register_type::<HoleSensor>();
        app.register_type::<HoleBoundingBox>();
        app.register_type::<HoleWalls>();

        app.register_type::<CurrentHole>();

        app.register_required_components::<PowerUp, CollidingEntities>();

        app.init_resource::<PhysicsConfig>();

        app.add_observer(on_hole_added);

        app.add_systems(OnEnter(CourseState::Waiting), (pause_physics, setup_course));
        app.add_systems(Update, test.in_set(LoadingCourseSystems));

        app.add_systems(OnEnter(CourseState::Playing), resume_physics);

        app.add_systems(OnEnter(HoleState::Playing), reset_player_position);
        app.add_systems(
            Update,
            (increment_score, log_score_changes).in_set(PlayingSystems),
        );

        app.add_systems(
            FixedUpdate,
            (
                handle_hole_sensors,
                handle_hole_bounding_box,
                current_hole_modified,
            )
                .in_set(PlayingSystems),
        );

        app.add_systems(OnEnter(HoleState::Completed), on_hole_completed);
        app.add_systems(
            OnEnter(CourseState::Completed),
            (remove_current_hole, on_course_completed),
        );
    }
}

fn remove_current_hole(mut commands: Commands) {
    commands.remove_resource::<CurrentHole>();
}

fn pause_physics(mut time: ResMut<Time<Physics>>) {
    time.pause();
}

fn resume_physics(mut time: ResMut<Time<Physics>>) {
    time.unpause();
}

fn reset_player_position(
    mut players: Query<(&mut Position, &mut LastPlayerPosition), With<Player>>,
    hole: Res<CurrentHole>,
) {
    for (mut position, mut last_position) in &mut players {
        position.0 = hole.hole.start_position.into();

        last_position.position = hole.hole.start_position;
        last_position.rotation = Quat::IDENTITY;
    }
}

fn on_course_completed(
    course_scene: Single<Entity, With<CourseSceneMarker>>,
    mut config: ResMut<GameConfig>,
    mut course_state: ResMut<NextState<CourseState>>,
    mut game_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    if let Ok(()) = config.next_course() {
        commands.entity(course_scene.into_inner()).despawn();
        course_state.set(CourseState::Waiting);
    } else {
        game_state.set(GameState::Completed);
    }
}

#[derive(Resource, Reflect, Default, Debug)]
pub(crate) struct GameConfig {
    courses: Vec<CourseDetails>,
    current: usize,
}

impl GameConfig {
    pub(crate) fn new(courses: Vec<CourseDetails>) -> Self {
        GameConfig {
            courses,
            current: 0,
        }
    }

    pub(crate) fn current(&self) -> &CourseDetails {
        &self.courses[self.current]
    }

    pub(crate) fn next_course(&mut self) -> Result<(), ()> {
        if self.current >= self.courses.len() - 1 {
            Err(())
        } else {
            self.current = self.current + 1;
            Ok(())
        }
    }
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub(crate) struct PhysicsConfig {
    floor: PhysicsParameters,
    walls: PhysicsParameters,
}

#[derive(Reflect)]
pub(crate) struct PhysicsParameters {
    friction: Friction,
    restitution: Restitution,
}

impl PhysicsParameters {
    pub(crate) fn default_components(&self) -> impl Bundle {
        (self.friction, self.restitution)
    }
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        PhysicsConfig {
            floor: PhysicsParameters {
                friction: Friction::new(0.9),
                restitution: Restitution::new(0.1),
            },
            walls: PhysicsParameters {
                friction: Friction::new(0.8).with_combine_rule(CoefficientCombine::Multiply),
                restitution: Restitution::new(0.9).with_combine_rule(CoefficientCombine::Max),
            },
        }
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

#[derive(Component, Reflect, Copy, Clone, Debug)]
#[require(
    RigidBody::Static,
    CollisionLayers::new(GameLayer::Default, [GameLayer::Default, GameLayer::Player]),
    Children)]
pub(crate) struct Hole {
    pub(crate) start_position: Vec3,
}

#[derive(Component, Reflect, Copy, Clone, Debug)]
#[require(
    RigidBody::Static,
    ColliderConstructor::Cuboid{ x_length: 0.2, y_length: 0.09, z_length: 0.2 },
    Sensor,
    CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
    CollidingEntities)]
pub(crate) struct HoleSensor {
    hole: Entity,
}

impl HoleSensor {
    pub(crate) fn new(hole: Entity) -> Self {
        HoleSensor { hole }
    }
}

#[derive(Component, Reflect, Copy, Clone, Debug)]
#[require(
    RigidBody::Static,
    Sensor,
    CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
    CollidingEntities)]
pub(crate) struct HoleBoundingBox {
    hole: Entity,
}

impl HoleBoundingBox {
    pub(crate) fn new(hole: Entity) -> Self {
        HoleBoundingBox { hole }
    }
}

#[derive(Component, Reflect, Debug)]
#[require(
    RigidBody::Static,
    CollisionLayers::new(GameLayer::Default, [GameLayer::Default, GameLayer::Player]))]
pub(crate) struct HoleWalls {
    hole_entity: Entity,
}

#[derive(Resource, Reflect, Debug)]
#[reflect(Resource)]
pub(crate) struct CurrentHole {
    pub(crate) hole: Hole,
    hole_entity: Entity,
    pub(crate) players: Vec<Player>,
}

#[derive(Component, Reflect, Debug)]
struct CourseSceneMarker;

fn setup_course(mut commands: Commands, server: Res<AssetServer>, config: Res<GameConfig>) {
    let course_id = &config.current().id;

    commands.spawn((
        Name::new("Course scene"),
        DynamicSceneRoot(server.load(format!("courses\\{course_id}.scn.ron"))),
        StateScoped(ServerState::Playing),
        CourseSceneMarker,
    ));
}

fn test(hole: Option<Res<CurrentHole>>, mut state: ResMut<NextState<CourseState>>) {
    if let Some(_) = hole {
        state.set(CourseState::Playing);
    }
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

                linear.0 = Vector::ZERO;
                angular.0 = Vector::ZERO;

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
    mut state: ResMut<NextState<HoleState>>,
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

    state.set(HoleState::Completed);
}

fn on_hole_completed(
    course: Query<&Course>,
    holes: Query<&Hole>,
    mut current_hole: ResMut<CurrentHole>,
    mut hole_state: ResMut<NextState<HoleState>>,
    mut course_state: ResMut<NextState<CourseState>>,
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
        course_state.set(CourseState::Completed);
        return;
    };

    let next_hole = holes.get(next_hole_entity).unwrap();
    current_hole.hole_entity = next_hole_entity;
    current_hole.hole = *next_hole;

    hole_state.set(HoleState::Playing);
}
