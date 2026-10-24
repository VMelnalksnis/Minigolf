use {
    crate::{
        Args, Lobby, PlayerDisconnected, PlayerJoinedLobby,
        game::{GameStarted, StartGame},
    },
    aeronet::io::{Session, bytes::Bytes, connection::LocalAddr, server::Server},
    aeronet_websocket::server::{ServerConfig, WebSocketServer},
    bevy::prelude::*,
    minigolf::{
        Player, PlayerCredentials,
        lobby::user::{ClientPacket, LobbyMember, PlayerInLobby, ServerPacket},
    },
    std::ops::RangeFull,
};

#[derive(Debug)]
pub(super) struct UserPlugin;

impl Plugin for UserPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, open_listener);

        app.add_observer(on_opened);
        app.add_observer(on_connected);
        app.add_observer(on_lobby_id_added);

        app.add_observer(on_player_joined_lobby);
        app.add_observer(on_player_disconnected);

        app.add_systems(Update, (handle_messages, game_started));
    }
}

#[derive(Debug, Component)]
struct UserListener;

#[derive(Debug, Component)]
struct UserSession;

fn open_listener(mut commands: Commands, args: Res<Args>) {
    let config = ServerConfig::builder()
        .with_bind_address(args.user_address)
        .with_no_encryption();

    let server = commands
        .spawn((Name::new("User listener"), UserListener))
        .queue(WebSocketServer::open(config))
        .id();

    info!("Opening server {server} for users");
}

fn on_opened(
    trigger: On<Add<Server>>,
    addresses: Query<&LocalAddr>,
    users: Query<&UserListener>,
) {
    let server = trigger.entity;
    let local_addr = addresses
        .get(server)
        .expect("opened server should have a binding socket `LocalAddr`");

    if let Ok(_) = users.get(server) {
        info!("{server} opened on {} for users", **local_addr);
    }
}

fn on_connected(
    trigger: On<Add<Session>>,
    mut sessions: Query<&mut Session>,
    servers: Query<&ChildOf>,
    users: Query<&UserListener>,
    mut commands: Commands,
) {
    let client = trigger.entity;
    let server = servers
        .get(client)
        .expect("connected session should have a server")
        .parent();

    if let Ok(_) = users.get(server) {
        info!("User {client} connected to {server}");

        let player = Player::new();
        let credentials = PlayerCredentials::default();
        commands
            .entity(client)
            .insert((player, credentials.clone(), UserSession));

        let message: String = ServerPacket::Hello(player.id, credentials).into();
        let mut session = sessions.get_mut(client).unwrap();
        session.send.push(Bytes::from_owner(message));
    }
}

fn handle_messages(
    mut sessions: Query<(Entity, &mut Session), With<UserSession>>,
    known_players: Query<(&Player, &PlayerCredentials)>,
    members: Query<&LobbyMember>,
    lobby_players: Query<(&Player, &LobbyMember)>,
    mut commands: Commands,
) {
    for (user_session, mut session) in &mut sessions {
        let session = &mut *session;

        // Components are inserted through deferred `Commands`, so packets handled later
        // in this loop would not see them in the queries; track the session state locally.
        let mut known_player = known_players
            .get(user_session)
            .ok()
            .map(|(player, credentials)| (*player, credentials.clone()));
        let mut lobby = members.get(user_session).ok().copied();

        for message in session.recv.drain(RangeFull::default()) {
            let client_packet = ClientPacket::from(message.payload.as_ref());
            info!("Client packet {client_packet:?}");

            match client_packet {
                ClientPacket::Hello => {
                    let (player, credentials) = match &known_player {
                        Some((player, credentials)) => (*player, credentials.clone()),
                        None => {
                            let player = Player::new();
                            let credentials = PlayerCredentials::default();

                            info!("New player {player:?}");

                            commands
                                .entity(user_session)
                                .insert((player, credentials.clone()));
                            known_player = Some((player, credentials.clone()));

                            (player, credentials)
                        }
                    };

                    let response: String =
                        ServerPacket::Hello(player.id, credentials.clone()).into();
                    session.send.push(Bytes::from_owner(response));
                }

                ClientPacket::CreateLobby => {
                    let lobby_member = LobbyMember::new();
                    let lobby_entity = commands
                        .spawn((Lobby::new(user_session), lobby_member))
                        .id();

                    let message: String = ServerPacket::LobbyCreated(lobby_member.lobby_id).into();
                    session.send.push(Bytes::from_owner(message));

                    commands.entity(lobby_entity).insert(lobby_member);
                    commands.entity(user_session).insert(lobby_member);
                    lobby = Some(lobby_member);
                }

                ClientPacket::JoinLobby(id) => {
                    let Some((player, _)) = &known_player else {
                        warn!("User {user_session} tried to join lobby {id} without a player");
                        continue;
                    };

                    let current_members = lobby_players
                        .iter()
                        .filter(|(_, l)| l.lobby_id == id)
                        .map(|(p, _)| p.id)
                        .collect::<Vec<_>>();

                    let message: String = ServerPacket::LobbyJoined(id, current_members).into();
                    session.send.push(Bytes::from_owner(message));

                    let lobby_member = LobbyMember::from(id);
                    commands.entity(user_session).insert(lobby_member);
                    commands.trigger(PlayerJoinedLobby(PlayerInLobby::new(id, player.id)));
                    lobby = Some(lobby_member);
                }

                ClientPacket::ListLobbies => {
                    let ids = members
                        .iter()
                        .map(|member| member.lobby_id)
                        .collect::<Vec<_>>();
                    let response: String = ServerPacket::AvailableLobbies(ids).into();
                    session.send.push(Bytes::from_owner(response));
                }

                ClientPacket::StartGame => {
                    let Some(user_lobby) = &lobby else {
                        warn!("User {user_session} tried to start a game without being in a lobby");
                        continue;
                    };
                    commands.trigger(StartGame::from(user_lobby));
                }

                ClientPacket::LeaveLobby => {
                    commands.entity(user_session).remove::<LobbyMember>();
                    lobby = None;
                }
            };
        }
    }
}

fn on_lobby_id_added(
    trigger: On<Add<LobbyMember>>,
    world: &World,
    lobby_ids: Query<(Entity, &LobbyMember)>,
) {
    let entity = trigger.entity;
    let (_, lobby_id) = lobby_ids.get(entity).unwrap();

    println!(
        "Added lobby id {:?} {:?}",
        lobby_id,
        world
            .inspect_entity(entity)
            .unwrap()
            .map(|(_, info)| info.name())
            .collect::<Vec<_>>()
    );

    lobby_ids
        .into_iter()
        .filter(|(_, id)| id.lobby_id == lobby_id.lobby_id)
        .for_each(|(e, id)| {
            println!(
                "Lobby {:?} contains {:?}",
                id,
                world
                    .inspect_entity(e)
                    .unwrap()
                    .map(|(_, info)| info.name())
                    .collect::<Vec<_>>()
            );
        });
}

fn game_started(
    mut game_started_reader: MessageReader<GameStarted>,
    mut members: Query<(&LobbyMember, &mut Session), With<UserSession>>,
) {
    for game_started in &mut game_started_reader.read() {
        for (id, mut session) in &mut members {
            if id.lobby_id != game_started.lobby_id {
                continue;
            }

            let message: String = ServerPacket::GameStarted(game_started.server.clone()).into();
            session.send.push(Bytes::from_owner(message));
        }
    }
}

fn on_player_joined_lobby(
    trigger: On<PlayerJoinedLobby>,
    mut sessions: Query<(&LobbyMember, &mut Session), With<UserSession>>,
) {
    let player = trigger.event();
    for (member, mut session) in &mut sessions {
        if member.lobby_id != player.lobby_id {
            continue;
        }

        let response: String = ServerPacket::PlayerJoined(player.0).into();
        session.send.push(Bytes::from_owner(response));
    }
}

fn on_player_disconnected(
    trigger: On<PlayerDisconnected>,
    mut sessions: Query<(&LobbyMember, &mut Session), With<UserSession>>,
) {
    let player = trigger.event();
    for (member, mut session) in &mut sessions {
        if member.lobby_id != player.lobby_id {
            continue;
        }

        let response: String = ServerPacket::PlayerLeft(player.0).into();
        session.send.push(Bytes::from_owner(response));
    }
}
