use {
    crate::{
        GameLayer, GlobalState,
        course::{
            Course, CurrentHole, Hole, HoleBoundingBox, HoleSensor, HoleWalls, PhysicsConfig,
            entities::{BallMagnet, Bumper, JumpPad},
        },
    },
    avian3d::prelude::*,
    bevy::prelude::*,
    bevy_replicon::prelude::*,
    minigolf::{LevelMesh, PlayableArea, PowerUp, PowerUpType},
    rand::Rng,
};

/// Plugin that handles course serialization to/from files
pub(crate) struct CourseSetupPlugin;

impl Plugin for CourseSetupPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CourseConfiguration>();
        app.init_resource::<CourseConfiguration>();

        app.register_type::<SpawnBumper>();
        app.add_event::<SpawnBumper>();

        app.register_type::<SpawnBlackHoleBumper>();
        app.add_event::<SpawnBlackHoleBumper>();

        app.add_observer(spawn_bumper_trigger); // todo
        app.add_observer(spawn_black_hole_bumper_trigger); // todo

        app.add_systems(
            Update,
            course_configuration_changed.run_if(resource_changed::<CourseConfiguration>),
        );
    }
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub(crate) struct CourseConfiguration {
    holes: Vec<HoleConfiguration>,
}

#[derive(Reflect)]
pub(crate) struct HoleConfiguration {
    transform: Transform,
    start_position: Vec3,

    hole_asset: String,
    wall_asset: String,

    bounding_box: Transform,
    hole_sensor: Transform,

    power_ups: Vec<Transform>,
    bumpers: Vec<Transform>,
    jump_pads: Vec<Transform>,
}

/// Updates [CourseConfiguration] resource with the current values of the course,
/// and it's child entities.
#[cfg(feature = "dev")]
pub(crate) fn capture_course_state(
    mut config: ResMut<CourseConfiguration>,
    course: Single<&Course>,
    holes: Query<(&Transform, &Hole, &LevelMesh, &Children), With<Hole>>,
    walls: Query<&LevelMesh, With<HoleWalls>>,
    bounding_box: Query<&Transform, With<HoleBoundingBox>>,
    hole_sensor: Query<&Transform, With<HoleSensor>>,
    power_ups: Query<&Transform, With<PowerUp>>,
    bumpers: Query<&Transform, With<Bumper>>,
    jump_pads: Query<&Transform, With<JumpPad>>,
) {
    config.holes = course
        .holes
        .iter()
        .map(|hole| {
            let (transform, hole, mesh, children) = holes.get(*hole).unwrap();

            let walls_mesh = map_single_component(children, walls);
            let bounding_transform = map_single_component(children, bounding_box);
            let sensor_transform = map_single_component(children, hole_sensor);

            HoleConfiguration {
                transform: transform.to_owned(),
                start_position: hole.start_position.to_owned(),

                hole_asset: mesh.asset.to_owned(),
                wall_asset: walls_mesh.asset,

                bounding_box: bounding_transform,
                hole_sensor: sensor_transform,

                power_ups: map_components(children, power_ups),
                bumpers: map_components(children, bumpers),
                jump_pads: map_components(children, jump_pads),
            }
        })
        .collect::<Vec<_>>();
}

#[cfg(feature = "dev")]
fn map_single_component<TComponent: Component + Clone, TTFilter: Component>(
    children: &Children,
    query: Query<&TComponent, With<TTFilter>>,
) -> TComponent {
    children
        .iter()
        .filter_map(|entity| query.get(entity).ok())
        .next()
        .unwrap()
        .to_owned()
}

#[cfg(feature = "dev")]
fn map_components<TComponent: Component + Clone, TTFilter: Component>(
    children: &Children,
    query: Query<&TComponent, With<TTFilter>>,
) -> Vec<TComponent> {
    children
        .iter()
        .filter_map(|entity| query.get(entity).ok())
        .map(|component| component.to_owned())
        .collect()
}

fn course_configuration_changed(
    config: Res<CourseConfiguration>,
    physics_config: Res<PhysicsConfig>,
    mut commands: Commands,
    server: Res<AssetServer>,
) {
    if config.holes.is_empty() {
        return;
    }

    let course = commands
        .spawn((
            Name::new("Course"),
            Course::new(),
            Transform::default(),
            Visibility::default(),
            Replicated,
            StateScoped(GlobalState::Game),
        ))
        .id();

    for (index, hole_config) in config.holes.iter().enumerate() {
        let floor_path = &hole_config.hole_asset;
        let floor_handle: Handle<Mesh> = server.load(floor_path);
        let walls_path = &hole_config.wall_asset;
        let walls_handle: Handle<Mesh> = server.load(walls_path);

        let hole_entity = commands
            .spawn((
                Name::new(format!("Hole {index}")),
                Hole {
                    start_position: hole_config.start_position,
                },
                hole_config.transform,
                PlayableArea,
                Replicated,
                Mesh3d(floor_handle),
                LevelMesh::from_path(floor_path),
                ColliderConstructor::TrimeshFromMeshWithConfig(TrimeshFlags::all()),
                ChildOf(course),
            ))
            .insert(physics_config.floor.default_components())
            .id();

        commands
            .spawn((
                Name::new(format!("Hole {index} walls")),
                Transform::IDENTITY,
                HoleWalls { hole_entity },
                Replicated,
                Mesh3d(walls_handle),
                LevelMesh::from_path(walls_path),
                ColliderConstructor::TrimeshFromMeshWithConfig(TrimeshFlags::all()),
                ChildOf(hole_entity),
            ))
            .insert(physics_config.walls.default_components());

        commands.spawn((
            Name::new(format!("Hole {index} bounding box")),
            hole_config.bounding_box,
            HoleBoundingBox::new(hole_entity),
            ColliderConstructor::Cuboid {
                x_length: 1.0,
                y_length: 1.0,
                z_length: 1.0,
            },
            ChildOf(hole_entity),
        ));

        commands.spawn((
            Name::new(format!("Hole {index} sensors")),
            hole_config.hole_sensor,
            HoleSensor::new(hole_entity),
            ChildOf(hole_entity),
        ));

        hole_config.power_ups.iter().for_each(|transform| {
            commands.spawn((
                Name::new("Power up"),
                *transform,
                Sensor,
                RigidBody::Static,
                CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
                ColliderConstructor::Sphere { radius: 0.1 },
                PowerUp::from(rand::rng().random::<PowerUpType>()),
                Replicated,
                ChildOf(hole_entity),
            ));
        });

        hole_config.bumpers.iter().for_each(|transform| {
            commands.spawn(bumper_bundle(
                Bumper::permanent(),
                transform.to_owned(),
                hole_entity,
            ));
        });

        hole_config.jump_pads.iter().for_each(|transform| {
            commands.spawn((
                Name::new("Jump pad"),
                JumpPad,
                *transform,
                RigidBody::Static,
                ColliderConstructor::Cylinder {
                    radius: 0.085344,
                    height: 0.05,
                },
                CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
                Sensor,
                Replicated,
                CollisionEventsEnabled,
                ChildOf(hole_entity),
            ));
        });
    }
}

#[derive(Event, Reflect, Debug)]
pub(crate) struct SpawnBumper {
    transform: Transform,
    permanent: bool,
}

impl SpawnBumper {
    /// Spawn a bumper which will despawn after certain amount of hits.
    pub(crate) fn with_hits(transform: Transform) -> Self {
        Self {
            transform,
            permanent: false,
        }
    }
}

const BUMPER_HITS: usize = 3; // todo: hits based on player count?

fn spawn_bumper_trigger(
    trigger: Trigger<SpawnBumper>,
    current_hole: Res<CurrentHole>,
    mut commands: Commands,
) {
    commands.spawn(bumper_bundle(
        match trigger.permanent {
            true => Bumper::permanent(),
            false => Bumper::with_hits(BUMPER_HITS),
        },
        trigger.transform,
        current_hole.hole_entity,
    ));
}

fn bumper_bundle(bumper: Bumper, transform: Transform, hole_entity: Entity) -> impl Bundle {
    let asset_path = "Entities.glb#Mesh1/Primitive0";

    (
        Name::new("Bumper"),
        bumper,
        transform,
        Replicated,
        LevelMesh::from_path(asset_path),
        ChildOf(hole_entity),
    )
}

#[derive(Event, Reflect, Debug)]
pub(crate) struct SpawnBlackHoleBumper {
    transform: Transform,
    permanent: bool,
}

impl SpawnBlackHoleBumper {
    /// Spawn a black hole bumper which will despawn after certain amount of hits.
    pub(crate) fn with_hits(transform: Transform) -> Self {
        Self {
            transform,
            permanent: false,
        }
    }
}

fn spawn_black_hole_bumper_trigger(
    trigger: Trigger<SpawnBlackHoleBumper>,
    current_hole: Res<CurrentHole>,
    mut commands: Commands,
) {
    let asset_path = "Entities.glb#Mesh1/Primitive0"; // todo: different from default bumper

    commands.spawn((
        Name::new("Bumper"),
        match trigger.permanent {
            true => Bumper::permanent(),
            false => Bumper::with_hits(BUMPER_HITS),
        },
        trigger.transform,
        Replicated,
        LevelMesh::from_path(asset_path),
        ChildOf(current_hole.hole_entity),
        children![(Name::new("Ball magnet"), BallMagnet::default(),)],
    ));
}
