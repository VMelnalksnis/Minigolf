use {
    aeronet::io::{
        Session,
        connection::{DisconnectReason, Disconnected, LocalAddr},
        server::Server,
    },
    aeronet_replicon::server::{AeronetRepliconServer, AeronetRepliconServerPlugin},
    aeronet_websocket::server::{WebSocketServer, WebSocketServerPlugin},
    aeronet_webtransport::{
        cert,
        server::{SessionRequest, SessionResponse, WebTransportServer, WebTransportServerPlugin},
        wtransport,
    },
    avian3d::prelude::*,
    bevy::{
        app::ScheduleRunnerPlugin,
        ecs::observer::TriggerTargets,
        prelude::*,
        render::{RenderPlugin, settings::WgpuSettings},
        winit::WinitPlugin,
    },
    bevy_replicon::prelude::*,
    core::time::Duration,
    minigolf::{LevelMesh, MinigolfPlugin, Player, PlayerInput, TICK_RATE},
};

const WEB_TRANSPORT_PORT: u16 = 25565;

const WEB_SOCKET_PORT: u16 = 25566;

#[derive(PhysicsLayer, Default)]
enum GameLayer {
    #[default]
    Default,
    Player,
}

/// `move_box` demo server
#[derive(Debug, Resource, clap::Parser)]
struct Args {
    /// Port to listen for WebTransport connections on
    #[arg(long, default_value_t = WEB_TRANSPORT_PORT)]
    wt_port: u16,
    /// Port to listen for WebSocket connections on
    #[arg(long, default_value_t = WEB_SOCKET_PORT)]
    ws_port: u16,
}

impl FromWorld for Args {
    fn from_world(_: &mut World) -> Self {
        <Self as clap::Parser>::parse()
    }
}

pub fn main() -> AppExit {
    App::new()
        .init_resource::<Args>()
        .add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        backends: None,
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .disable::<WinitPlugin>(),
        )
        .add_plugins((
            // transport
            WebTransportServerPlugin,
            WebSocketServerPlugin,
            // replication
            RepliconPlugins.set(ServerPlugin {
                // 1 frame lasts `1.0 / TICK_RATE` anyway
                tick_policy: TickPolicy::Manual,
                ..Default::default()
            }),
            AeronetRepliconServerPlugin,
            // game
            MinigolfPlugin,
            PhysicsPlugins::default(),
        ))
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / f64::from(TICK_RATE),
        )))
        .add_systems(
            Startup,
            (open_web_transport_server, open_web_socket_server, setup),
        )
        .add_observer(on_opened)
        .add_observer(on_session_request)
        .add_observer(on_connected)
        .add_observer(on_disconnected)
        .insert_resource(Time::<Fixed>::from_hz(128.0))
        .add_systems(
            FixedUpdate,
            recv_input.chain().run_if(server_or_singleplayer),
        )
        .add_systems(FixedUpdate, reset)
        .add_systems(FixedPreUpdate, bevy_replicon::server::increment_tick)
        .run()
}

fn reset(
    mut transforms: Query<
        (&mut Transform, &mut LinearVelocity, &mut AngularVelocity),
        With<Player>,
    >,
) {
    for (mut transform, mut linear, mut angular) in &mut transforms {
        if transform.translation.y < -0.15 {
            linear.x = 0.0;
            linear.y = 0.0;
            linear.z = 0.0;

            angular.x = 0.0;
            angular.y = 0.0;
            angular.z = 0.0;

            transform.translation = Vec3::new(0.0, 0.5, 0.0);
            info!("{transform:?}");
        }
    }
}

fn setup(mut commands: Commands, server: Res<AssetServer>) {
    let level_mesh_handle: Handle<Mesh> = server.load("Level1.glb#Mesh0/Primitive0");

    commands.spawn((
        LevelMesh {
            asset: "Level1.glb#Mesh0/Primitive0".parse().unwrap(),
        },
        Replicated,
        Transform::from_xyz(4.0, -1.0, 0.0).with_scale(Vec3::new(5.0, 1.0, 1.0)),
        RigidBody::Static,
        ColliderConstructor::TrimeshFromMeshWithConfig(TrimeshFlags::all()),
        Mesh3d(level_mesh_handle),
        CollisionLayers::new(GameLayer::Default, [GameLayer::Default, GameLayer::Player]),
        Friction::new(0.8).with_combine_rule(CoefficientCombine::Multiply),
        Restitution::new(0.7).with_combine_rule(CoefficientCombine::Multiply),
    ));
}

fn recv_input(
    mut commands: Commands,
    mut inputs: EventReader<FromClient<PlayerInput>>,
    mut children: Query<&PlayerSession>,
    mut players: Query<(&Transform, &mut PlayerInput)>,
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

        let Ok((transform, mut input)) = players.get_mut(session.player) else {
            continue;
        };

        *input = new_input.clone();

        info!("Input: {input:?}");
        info!("Position: {transform:?}");

        let force_vec = Vec3::new(input.movement.x, 0.0, input.movement.y).clamp_length_max(10.0);
        let impulse = ExternalImpulse::new(force_vec).with_persistence(false);
        commands.entity(session.player).insert(impulse);
    }
}

//
// WebTransport
//

fn open_web_transport_server(mut commands: Commands, args: Res<Args>) {
    let identity = wtransport::Identity::self_signed(["localhost", "127.0.0.1", "::1"])
        .expect("all given SANs should be valid DNS names");
    let cert = &identity.certificate_chain().as_slice()[0];
    let spki_fingerprint = cert::spki_fingerprint_b64(cert).expect("should be a valid certificate");
    let cert_hash = cert::hash_to_b64(cert.hash());
    info!("************************");
    info!("SPKI FINGERPRINT");
    info!("  {spki_fingerprint}");
    info!("CERTIFICATE HASH");
    info!("  {cert_hash}");
    info!("************************");

    let config = web_transport_config(identity, &args);
    let server = commands
        .spawn((Name::new("WebTransport Server"), AeronetRepliconServer))
        .queue(WebTransportServer::open(config))
        .id();
    info!("Opening WebTransport server {server}");
}

type WebTransportServerConfig = aeronet_webtransport::server::ServerConfig;

fn web_transport_config(identity: wtransport::Identity, args: &Args) -> WebTransportServerConfig {
    WebTransportServerConfig::builder()
        .with_bind_default(args.wt_port)
        .with_identity(identity)
        .keep_alive_interval(Some(Duration::from_secs(1)))
        .max_idle_timeout(Some(Duration::from_secs(5)))
        .expect("should be a valid idle timeout")
        .build()
}

fn on_session_request(mut request: Trigger<SessionRequest>, clients: Query<&Parent>) {
    let client = request.entity();
    let Ok(server) = clients.get(client).map(Parent::get) else {
        return;
    };

    info!("{client} connecting to {server} with headers:");
    for (header_key, header_value) in &request.headers {
        info!("  {header_key}: {header_value}");
    }

    request.respond(SessionResponse::Accepted);
}

//
// WebSocket
//

type WebSocketServerConfig = aeronet_websocket::server::ServerConfig;

fn open_web_socket_server(mut commands: Commands, args: Res<Args>) {
    let config = web_socket_config(&args);
    let server = commands
        .spawn((Name::new("WebSocket Server"), AeronetRepliconServer))
        .queue(WebSocketServer::open(config))
        .id();
    info!("Opening WebSocket server {server}");
}

fn web_socket_config(args: &Args) -> WebSocketServerConfig {
    WebSocketServerConfig::builder()
        .with_bind_default(args.ws_port)
        .with_no_encryption()
}

//
// server logic
//

fn on_opened(trigger: Trigger<OnAdd, Server>, servers: Query<&LocalAddr>) {
    let server = trigger.entity();
    let local_addr = servers
        .get(server)
        .expect("opened server should have a binding socket `LocalAddr`");
    info!("{server} opened on {}", **local_addr);
}

fn on_connected(trigger: Trigger<OnAdd, Session>, clients: Query<&Parent>, mut commands: Commands) {
    let client = trigger.entity();
    let Ok(server) = clients.get(client).map(Parent::get) else {
        return;
    };
    info!("{client} connected to {server}");

    let player = commands
        .spawn((
            Player,
            PlayerInput::default(),
            Name::new("Player"),
            Replicated,
            RigidBody::Dynamic,
            Collider::sphere(0.021336),
            CollisionLayers::new(GameLayer::Player, [GameLayer::Default]),
            Mass::from(0.04593),
            Transform::from_xyz(0.0, 0.5, 0.0),
            Friction::new(0.8).with_combine_rule(CoefficientCombine::Multiply),
            Restitution::new(0.7).with_combine_rule(CoefficientCombine::Multiply),
            AngularDamping(3.0),
        ))
        .id();

    commands.entity(client).insert(PlayerSession { player });
}

#[derive(Component, Reflect)]
struct PlayerSession {
    player: Entity,
}

fn on_disconnected(
    trigger: Trigger<Disconnected>,
    clients: Query<&Parent>,
    sessions: Query<&PlayerSession>,
    mut commands: Commands,
) {
    let client = trigger.entity();
    let Ok(server) = clients.get(client).map(Parent::get) else {
        return;
    };

    match &trigger.reason {
        DisconnectReason::User(reason) => {
            info!("{client} disconnected from {server} by user: {reason}");
        }
        DisconnectReason::Peer(reason) => {
            info!("{client} disconnected from {server} by peer: {reason}");
        }
        DisconnectReason::Error(err) => {
            warn!("{client} disconnected from {server} due to error: {err:?}");
        }
    }

    let Ok(session) = sessions.get(client) else {
        return;
    };

    commands.entity(session.player).despawn();
}
