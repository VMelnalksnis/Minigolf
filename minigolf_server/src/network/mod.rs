mod listeners;

use {
    crate::{
        ServerState,
        course::CurrentHole,
        network::listeners::ServerListenerPlugin,
        server::{Args, PlayerSession},
    },
    aeronet::{
        io::{
            Session,
            bytes::Bytes,
            connection::{Disconnect, Disconnected, LocalAddr},
            server::Server,
        },
        transport::AeronetTransportPlugin,
    },
    aeronet_replicon::server::AeronetRepliconServerPlugin,
    aeronet_websocket::client::{WebSocketClient, WebSocketClientPlugin},
    aeronet_webtransport::server::{SessionRequest, SessionResponse},
    bevy::prelude::*,
    bevy_replicon::prelude::*,
    core::time::Duration,
    minigolf::{
        AuthenticatePlayer, Player, PlayerCredentials, RequestAuthentication,
        lobby::{
            game::{ClientPacket, ServerPacket},
            user::LobbyMember,
        },
    },
};

/// Sets up minigolf server networking.
pub(crate) struct ServerNetworkPlugin;

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ServerListenerPlugin);
        app.add_plugins(WebSocketClientPlugin);
        app.add_plugins((AeronetTransportPlugin, AeronetRepliconServerPlugin));
        app.add_plugins(RepliconPlugins.set(ServerPlugin {
            tick_policy: TickPolicy::Manual,
            ..default()
        }));

        app.add_observer(on_opened)
            .add_observer(on_session_request)
            .add_observer(on_connected)
            .add_observer(on_disconnected)
            .add_event::<PlayerAuthenticated>();

        app.init_state::<ServerState>()
            .enable_state_scoped_entities::<ServerState>();

        app.init_resource::<LobbyServerConnector>()
            .configure_sets(
                Startup,
                LobbySet.run_if(in_state(ServerState::WaitingForLobby)),
            )
            .configure_sets(
                Update,
                LobbySet.run_if(in_state(ServerState::WaitingForLobby)),
            )
            .add_systems(Startup, lobby_setup.in_set(LobbySet))
            .add_systems(
                Update,
                (lobby_connection_messages, reconnect_to_lobby).in_set(LobbySet),
            );

        app.add_systems(OnEnter(ServerState::WaitingForGame), inform_lobby_server);

        app.configure_sets(
            Update,
            GameSet.run_if(in_state(ServerState::WaitingForGame)),
        )
        .add_systems(Update, game_setup_messages.in_set(GameSet));

        app.configure_sets(
            FixedUpdate,
            PlayersJoiningSet.run_if(in_state(ServerState::WaitingForPlayers)),
        )
        .add_systems(
            OnEnter(ServerState::WaitingForPlayers),
            setup_waiting_for_players,
        )
        .add_systems(
            FixedUpdate,
            (player_authentication_handler, all_players_joined).in_set(PlayersJoiningSet),
        )
        .register_type::<UnauthenticatedSession>();

        app.add_systems(OnEnter(ServerState::Playing), setup_observers);
        app.add_systems(OnExit(ServerState::Playing), disconnect_players);
    }
}

// Client setup for lobby server

#[derive(SystemSet, Clone, Eq, PartialEq, Hash, Debug)]
struct LobbySet;

#[derive(Resource, Reflect, Debug)]
struct LobbyServerConnector {
    timer: Timer,
    attempts: usize,
}

impl LobbyServerConnector {
    fn retry(&mut self) {
        if self.attempts >= 5 {
            panic!(
                "retried {} times to connect to lobby server without success",
                self.attempts
            );
        }

        self.attempts += 1;
        self.timer.reset();
        self.timer.unpause();
    }
}

impl FromWorld for LobbyServerConnector {
    fn from_world(_world: &mut World) -> Self {
        LobbyServerConnector {
            timer: Timer::new(Duration::from_secs(10), TimerMode::Once),
            attempts: 0,
        }
    }
}

fn lobby_setup(mut commands: Commands, args: Res<Args>) {
    commands.spawn((
        Name::new("Lobby server disconnect observer"),
        Observer::new(on_lobby_disconnected),
        StateScoped(ServerState::WaitingForLobby),
    ));

    connect_to_lobby(commands, args);
}

fn lobby_connection_messages(
    mut sessions: Query<&mut Session, With<WebSocketClient>>,
    mut server_state: ResMut<NextState<ServerState>>,
) {
    let Ok(mut session) = sessions.single_mut() else {
        return;
    };

    for message in session.recv.drain(..) {
        let server_packet = ServerPacket::from(message.payload.as_ref());
        info!("{server_packet:?}");

        match server_packet {
            ServerPacket::Hello => {
                server_state.set(ServerState::WaitingForGame);
            }

            _ => unimplemented!(),
        }
    }
}

fn on_lobby_disconnected(
    trigger: Trigger<Disconnected>,
    mut connector: ResMut<LobbyServerConnector>,
) {
    match trigger.event() {
        Disconnected::ByUser(reason) => {
            panic!("Disconnected from lobby server by user; {}", reason)
        }
        Disconnected::ByPeer(_) => connector.retry(),
        Disconnected::ByError(_) => connector.retry(),
    }
}

fn reconnect_to_lobby(
    mut connector: ResMut<LobbyServerConnector>,
    commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
) {
    connector.timer.tick(time.delta());

    if connector.timer.just_finished() {
        connect_to_lobby(commands, args);
    }
}

fn connect_to_lobby(mut commands: Commands, args: Res<Args>) {
    let config = aeronet_websocket::client::ClientConfig::builder().with_no_encryption();
    let target = format!("ws://{}", args.lobby_address);

    info!("Connecting to lobby server at {}", target);

    commands
        .spawn(Name::new("Lobby server connection"))
        .queue(WebSocketClient::connect(config, target));
}

fn inform_lobby_server(mut sessions: Query<&mut Session, With<WebSocketClient>>, args: Res<Args>) {
    let Ok(mut session) = sessions.single_mut() else {
        return;
    };

    let session = &mut *session;
    let address = args.get_publish_address();
    let response: String = ClientPacket::Available(address).into();
    session.send.push(Bytes::from_owner(response));
}

// game setup

#[derive(SystemSet, Clone, Eq, PartialEq, Hash, Debug)]
struct GameSet;

fn game_setup_messages(
    mut sessions: Query<&mut Session, With<WebSocketClient>>,
    mut server_state: ResMut<NextState<ServerState>>,
    mut commands: Commands,
) {
    let Ok(mut session) = sessions.single_mut() else {
        return;
    };

    let session = &mut *session;

    for message in session.recv.drain(..) {
        let server_packet = ServerPacket::from(message.payload.as_ref());
        info!("{server_packet:?}");

        match server_packet {
            ServerPacket::CreateGame(lobby_id, players) => {
                for (player_id, player_credentials) in players.into_iter() {
                    commands.spawn((
                        Name::new("Player"),
                        LobbyMember::from(lobby_id),
                        Player::from(player_id),
                        player_credentials,
                    ));
                }

                server_state.set(ServerState::WaitingForPlayers);
            }

            _ => unimplemented!(),
        }
    }
}

// waiting for players

#[derive(SystemSet, Clone, Eq, PartialEq, Hash, Debug)]
struct PlayersJoiningSet;

#[derive(Component, Reflect, Debug)]
struct UnauthenticatedSession;

fn setup_waiting_for_players(
    mut commands: Commands,
    mut sessions: Query<&mut Session, With<WebSocketClient>>,
    lobby_members: Query<&LobbyMember>,
) {
    info!("Waiting for players");

    commands.spawn((
        Name::new("Player session observer"),
        Observer::new(on_connected_while_waiting),
        StateScoped(ServerState::WaitingForPlayers),
    ));

    let lobby_id = lobby_members.iter().next().unwrap().lobby_id;
    let mut lobby_session = sessions.single_mut().unwrap();
    let message: String = ClientPacket::GameCreated(lobby_id).into();
    lobby_session.send.push(Bytes::from_owner(message));
}

fn on_connected_while_waiting(
    trigger: Trigger<OnAdd, Session>,
    parent: Query<&ChildOf>,
    sessions: Query<Entity, (With<Session>, Without<PlayerCredentials>)>,
    mut writer: EventWriter<ToClients<RequestAuthentication>>,
    mut commands: Commands,
) {
    let client = trigger.target();
    let Ok(_) = parent.get(client) else {
        warn!(
            "{:?} connected without parent while waiting for players",
            client
        );
        return;
    };

    commands.entity(client).insert(Replicated);

    info!("{:?} connected", client);
    let x = sessions.iter().collect::<Vec<_>>();
    info!("{:?} sessions", x);

    writer.write(ToClients {
        mode: SendMode::Direct(client),
        event: RequestAuthentication,
    });
}

fn player_authentication_handler(
    mut reader: EventReader<FromClient<AuthenticatePlayer>>,
    players: Query<(Entity, &Player, &PlayerCredentials)>,
    mut commands: Commands,
    mut writer: EventWriter<PlayerAuthenticated>,
) {
    info_once!("Listening for auth requests");

    for &FromClient {
        client_entity: session_entity,
        event: ref new_event,
    } in reader.read()
    {
        info!("Received auth request from {:?}", session_entity);

        let x = players
            .iter()
            .filter(|(_, player, _)| player.id == new_event.id)
            .map(|(entity, _, credentials)| (entity, credentials))
            .collect::<Vec<_>>();

        let &[(player_entity, creds)] = x.as_slice() else {
            commands.trigger_targets(Disconnect::new("Player id not found"), session_entity);
            warn!("player not found");
            break;
        };

        if *creds != new_event.credentials {
            commands.trigger_targets(Disconnect::new("Unauthorized"), session_entity);
            warn!("credentials don't match");
            break;
        }

        info!("User {:?} authenticated", player_entity);

        writer.write(PlayerAuthenticated {
            player: player_entity,
            session: session_entity,
        });
    }
}

fn all_players_joined(
    players: Query<(), With<Player>>,
    authenticated_players: Query<(), (With<Player>, With<Replicated>)>,
    current_hole: Option<Res<CurrentHole>>,
    mut state: ResMut<NextState<ServerState>>,
) {
    let total_player_count = players.iter().count();
    let connected_player_count = authenticated_players.iter().count();

    if let None = current_hole {
        return;
    }

    if total_player_count == connected_player_count {
        info!("All {:?} players joined", total_player_count);
        state.set(ServerState::Playing)
    }
}

#[derive(Event, Reflect, Debug)]
pub(crate) struct PlayerAuthenticated {
    pub(crate) player: Entity,
    pub(crate) session: Entity,
}

// logging

fn on_opened(trigger: Trigger<OnAdd, Server>, servers: Query<&LocalAddr>) {
    let server = trigger.target();
    let local_addr = servers
        .get(server)
        .expect("opened server should have a binding socket `LocalAddr`");
    info!("{server} opened on {}", **local_addr);
}

fn on_session_request(mut request: Trigger<SessionRequest>, clients: Query<&ChildOf>) {
    let client = request.target();
    let Ok(server) = clients.get(client).map(ChildOf::parent) else {
        return;
    };

    info!("{client} connecting to {server} with headers:");
    for (header_key, header_value) in &request.headers {
        info!("  {header_key}: {header_value}");
    }

    request.respond(SessionResponse::Accepted);
}

fn on_connected(
    trigger: Trigger<OnAdd, Session>,
    servers: Query<&ChildOf>,
    names: Query<&Name>,
    mut sessions: Query<&mut Session>,
) {
    let client = trigger.target();

    if let Ok(server) = servers.get(client).map(ChildOf::parent) {
        info!("{client} connected to {server}");
    } else if let Ok(name) = names.get(client) {
        info!("Connected to {name}");
        let mut session = sessions.get_mut(client).unwrap();

        let message: String = ClientPacket::Hello.into();
        session.send.push(Bytes::from_owner(message));
    } else {
        return;
    };
}

#[derive(Event, Reflect, Debug)]
struct PlayerDisconnected;

fn on_disconnected(
    trigger: Trigger<Disconnected>,
    servers: Query<&ChildOf>,
    names: Query<&Name>,
    mut commands: Commands,
) {
    let client = trigger.target();

    if let Ok(server) = servers.get(client).map(ChildOf::parent) {
        match trigger.event() {
            Disconnected::ByUser(reason) => {
                info!("{client} disconnected from {server} by user: {reason}");
            }
            Disconnected::ByPeer(reason) => {
                info!("{client} disconnected from {server} by peer: {reason}");
            }
            Disconnected::ByError(err) => {
                warn!("{client} disconnected from {server} due to error: {err:?}");
            }
        }

        commands.trigger_targets(PlayerDisconnected, client);
    } else if let Ok(name) = names.get(client) {
        match trigger.event() {
            Disconnected::ByUser(reason) => {
                info!("Disconnected from {name} by user: {reason}");
            }
            Disconnected::ByPeer(reason) => {
                info!("Disconnected from {name} by peer: {reason}");
            }
            Disconnected::ByError(err) => {
                warn!("Disconnected from {name} due to error: {err:?}");
            }
        }

        info!("Disconnected from {name}");
    } else {
        return;
    };
}

fn setup_observers(mut commands: Commands) {
    commands.spawn((
        Name::new("Player disconnection observer"),
        StateScoped(ServerState::Playing),
        Observer::new(on_player_disconnected),
    ));
}

fn on_player_disconnected(
    trigger: Trigger<PlayerDisconnected>,
    authenticated_players: Query<Entity, With<PlayerSession>>,
    mut next_state: ResMut<NextState<ServerState>>,
) {
    let player_entity = trigger.target();

    let remaining_players = authenticated_players
        .iter()
        .filter(|entity| *entity != player_entity)
        .collect::<Vec<_>>();

    if remaining_players.is_empty() {
        warn!("Zero players while still playing, ending game");
        next_state.set(ServerState::WaitingForGame);
    } else {
        info!("Remaining players {:?}", remaining_players);
    }
}

fn disconnect_players(players: Query<Entity, With<PlayerSession>>, mut commands: Commands) {
    for player in players.iter() {
        commands.trigger_targets(Disconnect::new("Game completed"), player);
    }
}
