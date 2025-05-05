use {
    crate::{
        config::ServerPlugin,
        course::{CoursePlugin, CurrentHole, HoleSensor, power_ups::ChipShotMarker},
        network::{PlayerAuthenticated, ServerNetworkPlugin},
    },
    aeronet::io::connection::Disconnected,
    avian3d::prelude::*,
    bevy::{math::DVec3, prelude::*},
    bevy_replicon::prelude::*,
    minigolf::{CourseDetails, MinigolfPlugin, Player, PlayerInput, PlayerPowerUps, PlayerScore},
    std::{
        net::{IpAddr, Ipv6Addr, SocketAddr},
        path::PathBuf,
    },
};

mod config;
mod course;
mod network;

fn main() -> AppExit {
    App::new()
        .init_resource::<Args>()
        .add_plugins(ServerPlugin)
        .add_plugins((
            ServerNetworkPlugin,
            MinigolfPlugin,
            PhysicsPlugins::default(),
            PhysicsDebugPlugin::default(),
        ))
        .add_plugins(StatesPlugin)
        .add_plugins(CoursePlugin)
        .add_observer(on_disconnected)
        .insert_resource(Time::<Fixed>::from_hz(128.0))
        .insert_resource(SubstepCount(8))
        .insert_resource(PhysicsLengthUnit(0.005))
        .insert_resource(DeactivationTime(0.2))
        .insert_resource(SleepingThreshold {
            angular: 10.0,
            linear: 1.0,
            ..default()
        })
        .register_type::<Configuration>()
        .init_resource::<Configuration>()
        .add_systems(Startup, load_configuration)
        .add_systems(FixedPreUpdate, bevy_replicon::server::increment_tick)
        .add_systems(FixedUpdate, recv_input.run_if(server_or_singleplayer))
        .add_systems(
            Update,
            on_player_authenticated.in_set(WaitingForPlayersSystems),
        )
        .add_systems(FixedUpdate, player_can_move.in_set(PlayingSystems))
        .add_systems(Update, (move_player, reset_can_move).in_set(PlayingSystems))
        .add_event::<ValidPlayerInput>()
        .run()
}

struct StatesPlugin;

impl Plugin for StatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<ServerState>();
        app.add_sub_state::<GameState>();
        app.add_sub_state::<CourseState>();
        app.add_sub_state::<HoleState>();

        app.register_type::<ServerState>();
        app.register_type::<GameState>();
        app.register_type::<CourseState>();
        app.register_type::<HoleState>();

        app.configure_sets(
            Update,
            ConnectingToLobbySystems.run_if(in_state(ServerState::WaitingForLobby)),
        );
        app.configure_sets(
            FixedUpdate,
            ConnectingToLobbySystems.run_if(in_state(ServerState::WaitingForLobby)),
        );

        app.configure_sets(
            Update,
            WaitingForGameSystems.run_if(in_state(ServerState::WaitingForGame)),
        );
        app.configure_sets(
            FixedUpdate,
            WaitingForGameSystems.run_if(in_state(ServerState::WaitingForGame)),
        );

        app.configure_sets(
            Update,
            WaitingForPlayersSystems.run_if(in_state(GameState::Waiting)),
        );
        app.configure_sets(
            FixedUpdate,
            WaitingForPlayersSystems.run_if(in_state(GameState::Waiting)),
        );

        app.configure_sets(
            Update,
            LoadingCourseSystems.run_if(in_state(CourseState::Waiting)),
        );
        app.configure_sets(
            FixedUpdate,
            LoadingCourseSystems.run_if(in_state(CourseState::Waiting)),
        );

        app.configure_sets(Update, PlayingSystems.run_if(in_state(HoleState::Playing)));
        app.configure_sets(
            FixedUpdate,
            PlayingSystems.run_if(in_state(HoleState::Playing)),
        );

        app.add_systems(
            OnEnter(GameState::Completed),
            |mut state: ResMut<NextState<ServerState>>| state.set(ServerState::WaitingForGame),
        );
    }
}

#[derive(States, Reflect, Default, Clone, Eq, PartialEq, Hash, Debug)]
#[states(scoped_entities)]
enum ServerState {
    #[default]
    WaitingForLobby,
    WaitingForGame,
    Playing,
}

/// The state of the current game, which consists of multiple courses.
#[derive(SubStates, Reflect, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[states(scoped_entities)]
#[source(ServerState = ServerState::Playing)]
enum GameState {
    /// Waiting for all the players to connect.
    #[default]
    Waiting,
    Playing,
    /// All the courses of the game are completed.
    Completed,
}

/// The state of the current course, which consists of multiple holes.
#[derive(SubStates, Reflect, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[states(scoped_entities)]
#[source(GameState = GameState::Playing)]
enum CourseState {
    /// Waiting for all the players to load.
    #[default]
    Waiting,
    Playing,
    /// All the holes of the course are completed.
    Completed,
}

/// The state of the current hole.
#[derive(SubStates, Reflect, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[states(scoped_entities)]
#[source(CourseState = CourseState::Playing)]
enum HoleState {
    #[default]
    Playing,
    /// All the players have completed the hole.
    Completed,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct ConnectingToLobbySystems;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct WaitingForGameSystems;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct WaitingForPlayersSystems;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct LoadingCourseSystems;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct PlayingSystems;

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
    pub(crate) web_transport_port: u16,

    /// Port to listen for WebSocket connections on
    #[arg(long, default_value_t = WEB_SOCKET_PORT)]
    pub(crate) web_socket_port: u16,

    /// Certificate to use for the WebSocket and WebTransport servers
    #[arg(long)]
    pub(crate) certificate_filepath: Option<PathBuf>,
    /// Private key for [certificate_filepath]
    #[arg(long)]
    pub(crate) private_key_filepath: Option<PathBuf>,

    /// Address to publish for clients to connect to the server
    #[arg(long)]
    pub(crate) publish_address: Option<String>,
    /// The address of the minigolf lobby server
    #[arg(long, default_value_t = LOBBY_ADDRESS)]
    pub(crate) lobby_address: SocketAddr,
}

impl Args {
    pub(crate) fn get_publish_address(&self) -> String {
        if let Some(address) = &self.publish_address {
            address.clone()
        } else {
            format!("ws://localhost:{}", &self.web_socket_port)
        }
    }
}

impl FromWorld for Args {
    fn from_world(_: &mut World) -> Self {
        <Self as clap::Parser>::parse()
    }
}

#[derive(Component, Reflect)]
pub(crate) struct PlayerSession {
    player: Entity,
}

#[derive(Resource, Reflect, Debug)]
#[reflect(Resource)]
pub(crate) struct Configuration {
    pub(crate) wind_strength: f32,

    pub(crate) hole_magnet_min_distance: f32,
    pub(crate) hole_magnet_max_distance: f32,
    pub(crate) hole_magnet_strength: f32,

    pub(crate) bumper_strength: f64,

    pub(crate) jump_pad_strength: f64,

    pub(crate) courses: Vec<CourseDetails>,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            wind_strength: 0.3,

            hole_magnet_min_distance: 0.05,
            hole_magnet_max_distance: 0.2,
            hole_magnet_strength: 50.0,

            bumper_strength: 0.1,

            jump_pad_strength: 0.2,

            courses: vec![],
        }
    }
}

fn load_configuration(server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn((
        Name::new("Configuration"),
        DynamicSceneRoot(server.load("config.scn.ron")),
    ));
}

#[derive(Event, Reflect, Debug)]
pub(crate) struct ValidPlayerInput {
    pub(crate) player: Entity,
    pub(crate) input: PlayerInput, // todo: need to handle different input types
}

#[derive(Component, Debug)]
pub(crate) struct LastPlayerPosition {
    pub(crate) position: Vec3,
    pub(crate) rotation: Quat,
}

fn recv_input(
    mut inputs: EventReader<FromClient<PlayerInput>>,
    mut sessions: Query<&PlayerSession>,
    mut players: Query<(&Player, &mut PlayerPowerUps)>,
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

        let (player, mut power_ups) = players.get_mut(session.player).unwrap();
        if input.is_movement() && !player.can_move {
            warn!(
                "Received player input from {:?} (player {:?}) when it cannot move",
                client_entity, player
            );
            continue;
        }

        if let Some(power_up_type) = input.get_power_up_type() {
            if !power_ups.get_power_ups().contains(&power_up_type) {
                warn!(
                    "Received player input with power up {:?} that the player {:?} does not have",
                    power_up_type, player
                );
                continue;
            }

            if let None = power_ups.use_power_up(power_up_type) {
                warn!(
                    "Could not use power up from input {:?} for player {:?}",
                    input, player
                );
                continue;
            }
        }

        writer.write(ValidPlayerInput {
            player: session.player,
            input: input.clone(),
        });
    }
}

fn move_player(
    mut reader: EventReader<ValidPlayerInput>,
    chip_shot: Query<&ChipShotMarker>,
    mut commands: Commands,
) {
    for &ValidPlayerInput { ref input, player } in reader.read() {
        let PlayerInput::Move(movement) = input else {
            continue;
        };

        let mut force_vec = Vec3::new(movement.x, 0.0, movement.y).clamp_length_max(10.0);
        force_vec.y = match chip_shot.get(player) {
            Ok(_) => force_vec.length(),
            Err(_) => 0.0,
        };

        commands
            .entity(player)
            .insert(ExternalImpulse::new(DVec3::from(force_vec)))
            .remove::<ChipShotMarker>();
    }
}

fn reset_can_move(mut reader: EventReader<ValidPlayerInput>, mut players: Query<&mut Player>) {
    for input in reader.read() {
        let PlayerInput::Move(_) = input.input else {
            continue;
        };

        players.get_mut(input.player).unwrap().can_move = false;
    }
}

fn player_can_move(
    mut player_velocity: Query<
        (Entity, &mut Player, &Transform, &mut LastPlayerPosition),
        Added<Sleeping>,
    >,
    holes: Query<&CollidingEntities, With<HoleSensor>>,
    mut current_hole: ResMut<CurrentHole>,
) {
    for (entity, mut player, transform, mut position) in &mut player_velocity {
        let is_in_hole = holes.iter().any(|h| h.contains(&entity));

        player.can_move = !is_in_hole;

        if player.can_move {
            position.position = transform.translation;
            position.rotation = transform.rotation;

            info!("Last position: {position:?}");
        } else if is_in_hole {
            info!("Player {:?} completed the hole", entity);
            current_hole.players.push(*player);
        }
    }
}

fn on_player_authenticated(mut reader: EventReader<PlayerAuthenticated>, mut commands: Commands) {
    for authenticated in reader.read() {
        commands.entity(authenticated.player).insert((
            LastPlayerPosition {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            PlayerScore::default(),
            PlayerPowerUps::default(),
            Replicated,
            RigidBody::Dynamic,
            Collider::sphere(0.021336),
            CollisionLayers::new(GameLayer::Player, [GameLayer::Default]),
            Mass::from(0.04593),
            Transform::from_translation(Vec3::ZERO),
            Friction::new(0.2),
            Restitution::new(0.99),
            AngularDamping(1.0),
            LinearDamping(0.5),
            SweptCcd::default(),
            CollisionEventsEnabled,
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
    let client = trigger.target();
    info!("Disconnected {:?}", client);
    let Ok(session) = sessions.get(client) else {
        return;
    };

    commands.entity(session.player).despawn();
}
