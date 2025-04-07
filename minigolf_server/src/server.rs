use {
    crate::network::ServerNetworkPlugin,
    aeronet::io::{Session, connection::Disconnected},
    avian3d::prelude::*,
    bevy::{
        app::ScheduleRunnerPlugin,
        ecs::observer::TriggerTargets,
        prelude::*,
        render::{RenderPlugin, settings::WgpuSettings},
        winit::WinitPlugin,
    },
    bevy_replicon::{prelude::*, server::increment_tick},
    core::time::Duration,
    minigolf::{LevelMesh, MinigolfPlugin, Player, PlayerInput, TICK_RATE},
    std::{
        convert::Into,
        net::{IpAddr, Ipv6Addr, SocketAddr},
    },
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
            ServerNetworkPlugin,
            MinigolfPlugin,
            PhysicsPlugins::default(),
        ))
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / f64::from(TICK_RATE),
        )))
        .add_systems(Startup, setup)
        .add_observer(on_connected)
        .add_observer(on_disconnected)
        .insert_resource(Time::<Fixed>::from_hz(128.0))
        .add_systems(FixedUpdate, recv_input.run_if(server_or_singleplayer))
        .add_systems(FixedUpdate, reset)
        .add_systems(FixedPreUpdate, increment_tick)
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
// server logic
//

fn on_connected(trigger: Trigger<OnAdd, Session>, parent: Query<&Parent>, mut commands: Commands) {
    let client = trigger.entity();
    let Ok(_) = parent.get(client) else {
        return;
    };

    let player = commands
        .spawn((
            Player::new(),
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

fn on_disconnected(
    trigger: Trigger<Disconnected>,
    sessions: Query<&PlayerSession>,
    mut commands: Commands,
) {
    let client = trigger.entity();
    let Ok(session) = sessions.get(client) else {
        return;
    };

    commands.entity(session.player).despawn();
}
