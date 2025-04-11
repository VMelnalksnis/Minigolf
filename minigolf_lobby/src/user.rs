use {
    crate::{
        Args, Lobby,
        game::{GameStarted, StartGame},
    },
    aeronet::{io::Session, io::bytes::Bytes, io::connection::LocalAddr, io::server::Server},
    aeronet_websocket::server::{ServerConfig, WebSocketServer},
    bevy::prelude::*,
    minigolf::{
        Player, PlayerCredentials,
        lobby::{LobbyMember, UserClientPacket, UserServerPacket},
    },
    std::ops::RangeFull,
};

#[derive(Debug)]
pub(super) struct UserPlugin;

impl Plugin for UserPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, open_listener)
            .add_observer(on_opened)
            .add_observer(on_connected)
            .add_observer(on_lobby_id_added)
            .add_systems(Update, handle_messages)
            .add_systems(Update, game_started);
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
    trigger: Trigger<OnAdd, Server>,
    addresses: Query<&LocalAddr>,
    users: Query<&UserListener>,
) {
    let server = trigger.entity();
    let local_addr = addresses
        .get(server)
        .expect("opened server should have a binding socket `LocalAddr`");

    if let Ok(_) = users.get(server) {
        info!("{server} opened on {} for users", **local_addr);
    }
}

fn on_connected(
    trigger: Trigger<OnAdd, Session>,
    mut sessions: Query<&mut Session>,
    servers: Query<&Parent>,
    users: Query<&UserListener>,
    mut commands: Commands,
) {
    let client = trigger.entity();
    let server = servers
        .get(client)
        .expect("connected session should have a server")
        .get();

    if let Ok(_) = users.get(server) {
        info!("User {client} connected to {server}");

        let player = Player::new();
        let credentials = PlayerCredentials::default();
        commands
            .entity(client)
            .insert((player, credentials.clone(), UserSession));

        let message: String = UserServerPacket::Hello(player.id, credentials).into();
        let mut session = sessions.get_mut(client).unwrap();
        session.send.push(Bytes::from_owner(message));
    }
}

fn handle_messages(
    mut start_game_writer: EventWriter<StartGame>,
    mut sessions: Query<(Entity, &mut Session), With<UserSession>>,
    known_players: Query<(&Player, &PlayerCredentials)>,
    members: Query<&LobbyMember>,
    mut commands: Commands,
) {
    for (user_session, mut session) in &mut sessions {
        let session = &mut *session;

        for message in session.recv.drain(RangeFull::default()) {
            let client_packet = UserClientPacket::from(message.payload.as_ref());
            info!("Client packet {client_packet:?}");

            match client_packet {
                UserClientPacket::Hello => {
                    let (player, credentials) = match known_players.get(user_session) {
                        Ok((player, credentials)) => (player.clone(), credentials.clone()),
                        Err(_) => {
                            let player = Player::new();
                            let credentials = PlayerCredentials::default();

                            info!("New player {player:?}");

                            commands
                                .entity(user_session)
                                .insert((player, credentials.clone()));

                            (player, credentials)
                        }
                    };

                    let response: String =
                        UserServerPacket::Hello(player.id, credentials.clone()).into();
                    session.send.push(Bytes::from_owner(response));
                }

                UserClientPacket::CreateLobby => {
                    let lobby_member = LobbyMember::new();
                    let lobby = commands
                        .spawn((Lobby::new(user_session), lobby_member))
                        .id();

                    let message: String =
                        UserServerPacket::LobbyCreated(lobby_member.lobby_id).into();
                    session.send.push(Bytes::from_owner(message));

                    commands.entity(lobby).insert(lobby_member);
                    commands.entity(user_session).insert(lobby_member);

                    // start_game_writer.send(StartGame {
                    //     lobby_id: lobby_member.lobby_id,
                    // });
                }

                UserClientPacket::JoinLobby(id) => {
                    commands.entity(user_session).insert(LobbyMember::from(id));

                    let message: String = UserServerPacket::LobbyJoined(id).into();
                    session.send.push(Bytes::from_owner(message));

                    start_game_writer.send(StartGame { lobby_id: id });
                }

                UserClientPacket::ListLobbies => {
                    let ids = members
                        .iter()
                        .map(|member| member.lobby_id)
                        .collect::<Vec<_>>();
                    let response: String = UserServerPacket::AvailableLobbies(ids).into();
                    session.send.push(Bytes::from_owner(response));
                }

                UserClientPacket::StartGame => {
                    let user_lobby = members.get(user_session).unwrap();
                    start_game_writer.send(user_lobby.into());
                }
                UserClientPacket::LeaveLobby => {
                    commands.entity(user_session).remove::<LobbyMember>();
                }
            };
        }
    }
}

fn on_lobby_id_added(
    trigger: Trigger<OnAdd, LobbyMember>,
    world: &World,
    lobby_ids: Query<(Entity, &LobbyMember)>,
) {
    let entity = trigger.entity();
    let (_, lobby_id) = lobby_ids.get(entity).unwrap();

    println!(
        "Added lobby id {:?} {:?}",
        lobby_id,
        world
            .inspect_entity(entity)
            .map(|info| info.name())
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
                    .map(|info| info.name())
                    .collect::<Vec<_>>()
            );
        });
}

fn game_started(
    mut game_started_reader: EventReader<GameStarted>,
    mut members: Query<(&LobbyMember, &mut Session), With<UserSession>>,
) {
    for game_started in &mut game_started_reader.read() {
        for (id, mut session) in &mut members {
            if id.lobby_id != game_started.lobby_id {
                continue;
            }

            let message: String = UserServerPacket::GameStarted(game_started.server.clone()).into();
            session.send.push(Bytes::from_owner(message));
        }
    }
}
