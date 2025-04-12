use bevy::input::common_conditions::input_toggle_active;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use {
    crate::{
        config::ServerPlugin,
        network::{PlayerAuthenticated, ServerNetworkPlugin},
    },
    aeronet::io::connection::Disconnected,
    avian3d::prelude::*,
    bevy::{ecs::observer::TriggerTargets, prelude::*},
    bevy_replicon::{prelude::*, server::increment_tick},
    minigolf::{LevelMesh, MinigolfPlugin, Player, PlayerInput},
    std::net::{IpAddr, Ipv6Addr, SocketAddr},
};

const WEB_TRANSPORT_PORT: u16 = 25565;

const WEB_SOCKET_PORT: u16 = 25566;

const LOBBY_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 25568);

#[derive(PhysicsLayer, Default)]
enum GameLayer {
    #[default]
    Default,
    Player,
}

/// `move_box` demo server
#[derive(Debug, Resource, clap::Parser)]
pub(crate) struct Args {
    /// Port to listen for WebTransport connections on
    #[arg(long, default_value_t = WEB_TRANSPORT_PORT)]
    pub(crate) wt_port: u16,
    /// Port to listen for WebSocket connections on
    #[arg(long, default_value_t = WEB_SOCKET_PORT)]
    pub(crate) ws_port: u16,
    /// The address of the minigolf lobby server
    #[arg(long, default_value_t = LOBBY_ADDRESS)]
    pub(crate) lobby_address: SocketAddr,
}

impl FromWorld for Args {
    fn from_world(_: &mut World) -> Self {
        <Self as clap::Parser>::parse()
    }
}

#[derive(Component, Reflect)]
struct PlayerSession {
    player: Entity,
}

pub fn main() -> AppExit {
    App::new()
        .init_resource::<Args>()
        .add_plugins(ServerPlugin)
        .add_plugins((
            ServerNetworkPlugin,
            MinigolfPlugin,
            PhysicsPlugins::default(),
            PhysicsDebugPlugin::default(),
        ))
        .add_systems(Startup, setup)
        .add_observer(on_disconnected)
        .insert_resource(Time::<Fixed>::from_hz(128.0))
        .insert_resource::<DeactivationTime>(DeactivationTime(0.2))
        .insert_resource::<SleepingThreshold>(SleepingThreshold {
            angular: 1.0,
            ..default()
        })
        .add_systems(FixedUpdate, recv_input.run_if(server_or_singleplayer))
        .add_systems(FixedUpdate, reset)
        .add_systems(FixedUpdate, player_can_move)
        .add_systems(FixedPreUpdate, increment_tick)
        .add_systems(Update, on_player_authenticated)
        .add_observer(on_hole_added)
        .register_type::<Course>()
        .register_type::<Hole>()
        .init_state::<CourseState>()
        .run()
}

#[derive(Component, Debug)]
struct LastPlayerPosition {
    position: Vec3,
    rotation: Quat,
}

fn reset(
    mut transforms: Query<
        (
            &mut Transform,
            &mut LinearVelocity,
            &mut AngularVelocity,
            &LastPlayerPosition,
        ),
        With<Player>,
    >,
) {
    for (mut transform, mut linear, mut angular, last_position) in &mut transforms {
        if transform.translation.y < -0.15 {
            linear.x = 0.0;
            linear.y = 0.0;
            linear.z = 0.0;

            angular.x = 0.0;
            angular.y = 0.0;
            angular.z = 0.0;

            info!("Last position: {last_position:?}");
            transform.translation = last_position.position;
            transform.rotation = last_position.rotation;
            info!("{transform:?}");
        }
    }
}

#[derive(Component, Reflect, Debug)]
struct Course {
    holes: Vec<Entity>,
}

impl Course {
    fn new() -> Self {
        Course { holes: vec![] }
    }
}

#[derive(States, Default, Clone, Eq, PartialEq, Hash, Debug)]
enum CourseState {
    #[default]
    WaitingForGame,
    WaitingForPlayers,
    Playing,
    Completed,
}

#[derive(Component, Reflect, Debug)]
struct Hole {
    start_position: Vec3,
}

#[derive(Component, Reflect, Debug)]
struct PlayerScore {
    score: u32,
}

impl Hole {
    fn new() -> Self {
        Hole {
            start_position: Vec3::ZERO,
        }
    }
}

fn setup(mut commands: Commands, server: Res<AssetServer>) {
    let scene = commands
        .spawn((Name::new("Scene"), SceneRoot::default()))
        .id();

    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        Transform::from_xyz(-5.0, 2.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

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

    commands
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
        .set_parent(course);
}

fn on_hole_added(trigger: Trigger<OnAdd, Hole>, mut course: Query<&mut Course>) {
    let hole_entity = trigger.entity();
    let mut course = course.single_mut();
    course.holes.push(hole_entity);
}

fn recv_input(
    mut commands: Commands,
    mut inputs: EventReader<FromClient<PlayerInput>>,
    mut children: Query<&PlayerSession>,
    mut players: Query<(&Transform, &mut PlayerInput, &Player)>,
) {
    for &FromClient {
        client_entity,
        event: ref new_input,
    } in inputs.read()
    {
        info!("Entity: {client_entity:?}");
        for component in client_entity.components() {
            info!("Component: {component:?}");
        }
        for component in client_entity.entities() {
            info!("Entity: {component:?}");
        }

        let Ok(session) = children.get_mut(client_entity) else {
            continue;
        };

        let Ok((transform, mut input, player)) = players.get_mut(session.player) else {
            continue;
        };

        if !player.can_move {
            continue;
        }

        *input = new_input.clone();

        info!("Input: {input:?}");
        info!("Position: {transform:?}");

        let force_vec = Vec3::new(input.movement.x, 0.0, input.movement.y).clamp_length_max(10.0);
        let impulse = ExternalImpulse::new(force_vec).with_persistence(false);
        commands.entity(session.player).insert(impulse);
    }
}

fn player_can_move(
    mut player_velocity: Query<
        (
            &LinearVelocity,
            &mut Player,
            &Transform,
            &mut LastPlayerPosition,
        ),
        Changed<LinearVelocity>,
    >,
) {
    for (velocity, mut player, transform, mut position) in &mut player_velocity {
        player.can_move = *velocity == LinearVelocity::ZERO;

        if player.can_move {
            position.position = transform.translation;
            position.rotation = transform.rotation;

            // info!("Last position: {position:?}");
        }
    }
}

//
// server logic
//

fn on_player_authenticated(mut reader: EventReader<PlayerAuthenticated>, mut commands: Commands) {
    for authenticated in reader.read() {
        let initial_position = Vec3::new(0.0, 0.5, 0.0);

        commands.entity(authenticated.player).insert((
            PlayerInput::default(),
            LastPlayerPosition {
                position: initial_position,
                rotation: Quat::IDENTITY,
            },
            Replicated,
            RigidBody::Dynamic,
            Collider::sphere(0.021336),
            CollisionLayers::new(GameLayer::Player, [GameLayer::Default]),
            Mass::from(0.04593),
            Transform::from_translation(initial_position),
            Friction::new(0.8).with_combine_rule(CoefficientCombine::Multiply),
            Restitution::new(0.7).with_combine_rule(CoefficientCombine::Multiply),
            AngularDamping(3.0),
        ));

        commands
            .entity(authenticated.session)
            .insert(PlayerSession {
                player: authenticated.player,
            });
    }
}

fn on_disconnected(
    trigger: Trigger<Disconnected>,
    sessions: Query<&PlayerSession>,
    mut commands: Commands,
) {
    let client = trigger.entity();
    info!("Disconnected {:?}", client);
    let Ok(session) = sessions.get(client) else {
        return;
    };

    commands.entity(session.player).despawn();
}
