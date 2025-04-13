use {
    crate::{
        config::ServerPlugin,
        course::{CoursePlugin, HoleSensor, PlayerScore},
        network::{PlayerAuthenticated, ServerNetworkPlugin},
    },
    aeronet::io::connection::Disconnected,
    avian3d::prelude::*,
    bevy::prelude::*,
    bevy_replicon::{prelude::*, server::increment_tick},
    minigolf::{MinigolfPlugin, Player, PlayerInput},
    std::net::{IpAddr, Ipv6Addr, SocketAddr},
};

const WEB_TRANSPORT_PORT: u16 = 25565;

const WEB_SOCKET_PORT: u16 = 25566;

const LOBBY_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 25568);

#[derive(PhysicsLayer, Default)]
pub(crate) enum GameLayer {
    #[default]
    Default,
    Player,
}

/// minigolf server
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
        .add_plugins(CoursePlugin)
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
        .add_systems(Update, (move_player, reset_can_move))
        .add_event::<ValidPlayerInput>()
        .run()
}

#[derive(Event, Reflect, Debug)]
pub(crate) struct ValidPlayerInput {
    pub(crate) player: Entity,
    input: PlayerInput,
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
        if transform.translation.z.abs() > 1.1
            || transform.translation.x < -1.1
            || transform.translation.x > 9.5
        {
            linear.0 = Vec3::ZERO;
            angular.0 = Vec3::ZERO;

            info!("Last position: {last_position:?}");
            transform.translation = last_position.position;
            transform.rotation = last_position.rotation;
            info!("{transform:?}");
        }
    }
}

fn recv_input(
    mut inputs: EventReader<FromClient<PlayerInput>>,
    mut sessions: Query<&PlayerSession>,
    mut players: Query<&Player>,
    mut writer: EventWriter<ValidPlayerInput>,
) {
    for &FromClient {
        client_entity,
        event: ref input,
    } in inputs.read()
    {
        let Ok(session) = sessions.get_mut(client_entity) else {
            warn!(
                "Received player input from {:?} without a session",
                client_entity
            );
            continue;
        };

        let player = players.get_mut(session.player).unwrap();
        if !player.can_move {
            warn!(
                "Received player input from {:?} (player {:?}) when it cannot move",
                client_entity, player
            );
            continue;
        }

        writer.send(ValidPlayerInput {
            player: session.player,
            input: input.clone(),
        });
    }
}

fn move_player(mut reader: EventReader<ValidPlayerInput>, mut commands: Commands) {
    for &ValidPlayerInput { ref input, player } in reader.read() {
        let force_vec = Vec3::new(input.movement.x, 0.0, input.movement.y).clamp_length_max(10.0);

        commands
            .entity(player)
            .insert(ExternalImpulse::new(force_vec));
    }
}

fn reset_can_move(mut reader: EventReader<ValidPlayerInput>, mut players: Query<&mut Player>) {
    for input in reader.read() {
        players.get_mut(input.player).unwrap().can_move = false;
    }
}

fn player_can_move(
    mut player_velocity: Query<
        (Entity, &mut Player, &Transform, &mut LastPlayerPosition),
        Added<Sleeping>,
    >,
    holes: Query<&CollidingEntities, With<HoleSensor>>,
) {
    for (entity, mut player, transform, mut position) in &mut player_velocity {
        let is_in_hole = holes.single().contains(&entity);

        player.can_move = !is_in_hole;

        if player.can_move {
            position.position = transform.translation;
            position.rotation = transform.rotation;

            info!("Last position: {position:?}");
        } else if is_in_hole {
            info!("Player {:?} completed the hole", entity);
        }
    }
}

fn on_player_authenticated(mut reader: EventReader<PlayerAuthenticated>, mut commands: Commands) {
    for authenticated in reader.read() {
        let initial_position = Vec3::new(0.0, 0.5, 0.0);

        commands.entity(authenticated.player).insert((
            PlayerInput::default(),
            LastPlayerPosition {
                position: initial_position,
                rotation: Quat::IDENTITY,
            },
            PlayerScore::default(),
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
