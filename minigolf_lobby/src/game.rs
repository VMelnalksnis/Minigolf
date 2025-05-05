use {
    crate::Args,
    aeronet::io::{Session, bytes::Bytes, connection::LocalAddr, server::Server},
    aeronet_websocket::server::{ServerConfig, WebSocketServer},
    bevy::prelude::*,
    minigolf::{
        lobby::{
            LobbyId,
            game::{ClientPacket, CreateGameRequest, ServerPacket},
            user::LobbyMember,
        },
        {Player, PlayerCredentials},
    },
};

#[derive(Debug)]
pub(super) struct GameServerPlugin;

impl Plugin for GameServerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<StartGame>();

        app.add_systems(Startup, open_listener);

        app.add_observer(on_opened);
        app.add_observer(on_connected);
        app.add_observer(on_game_server_added);
        app.add_observer(on_game_server_removed);
        app.add_observer(on_start_game);

        app.add_systems(Update, handle_messages);

        app.add_event::<GameStarted>();
    }
}

#[derive(Debug, Component)]
struct GamerServerListener;

#[derive(Debug, Component)]
struct GameServerSession;

#[derive(Debug, Component)]
struct GameServer {
    address: String,
}

fn open_listener(mut commands: Commands, args: Res<Args>) {
    let config = ServerConfig::builder()
        .with_bind_address(args.game_address)
        .with_no_encryption();

    let server = commands
        .spawn((Name::new("Game server listener"), GamerServerListener))
        .queue(WebSocketServer::open(config))
        .id();

    info!("Opening server {server} for game servers");
}

fn on_opened(
    trigger: Trigger<OnAdd, Server>,
    addresses: Query<&LocalAddr>,
    games: Query<&GamerServerListener>,
) {
    let server = trigger.target();
    let local_addr = addresses
        .get(server)
        .expect("opened server should have a binding socket `LocalAddr`");

    if let Ok(_) = games.get(server) {
        info!("{server} opened on {} for game servers", **local_addr);
    }
}

fn on_connected(
    trigger: Trigger<OnAdd, Session>,
    servers: Query<&ChildOf>,
    games: Query<&GamerServerListener>,
    mut commands: Commands,
) {
    let client = trigger.target();
    let server = servers
        .get(client)
        .expect("connected session should have a server")
        .parent();

    if let Ok(_) = games.get(server) {
        info!("Game server {client} connected to {server}");
        commands.entity(client).insert(GameServerSession);
    }
}

fn handle_messages(
    mut sessions: Query<(Entity, &mut Session), With<GameServerSession>>,
    mut game_started_writer: EventWriter<GameStarted>,
    game_servers: Query<&GameServer>,
    mut commands: Commands,
) {
    for (server_entity, mut session) in &mut sessions {
        let session = &mut *session;

        for message in session.recv.drain(..) {
            let client_packet = ClientPacket::from(message.payload.as_ref());
            info!("{client_packet:?}");

            match &client_packet {
                ClientPacket::Hello => {
                    let response: String = ServerPacket::Hello.into();
                    session.send.push(Bytes::from_owner(response));
                }

                ClientPacket::Available(game_server_address) => {
                    commands.entity(server_entity).insert(GameServer {
                        address: game_server_address.clone(),
                    });
                }

                ClientPacket::Busy => {
                    commands.entity(server_entity).remove::<GameServer>();
                }

                ClientPacket::GameCreated(lobby_id) => {
                    let server = game_servers.get(server_entity).unwrap();
                    game_started_writer.write(GameStarted {
                        lobby_id: *lobby_id,
                        server: server.address.clone(),
                    });
                }
            }

            match client_packet {
                ClientPacket::Available(_) => {}
                _ => {
                    commands.entity(server_entity).remove::<GameServer>();
                }
            };
        }
    }
}

fn on_game_server_added(trigger: Trigger<OnAdd, GameServer>, servers: Query<&GameServer>) {
    let connected_server = servers.get(trigger.target()).unwrap();
    let all_servers = &servers.iter().collect::<Vec<_>>();

    info!("Added new game server {connected_server:?}, all servers {all_servers:?}");
}

fn on_game_server_removed(
    trigger: Trigger<OnRemove, GameServer>,
    servers: Query<(Entity, &GameServer)>,
) {
    let remaining = &servers
        .iter()
        .filter(|(e, _)| *e != trigger.target())
        .map(|(_, s)| s)
        .collect::<Vec<_>>();

    info!("Removed game server, remaining {remaining:?}");
}

#[derive(Event, Reflect, Debug)]
pub(crate) struct StartGame {
    pub(crate) lobby_id: LobbyId,
}

impl From<&LobbyMember> for StartGame {
    fn from(value: &LobbyMember) -> Self {
        StartGame {
            lobby_id: value.lobby_id,
        }
    }
}

fn on_start_game(
    trigger: Trigger<StartGame>,
    mut servers: Query<&mut Session, With<GameServer>>,
    lobby_players: Query<(&LobbyMember, &Player, &PlayerCredentials)>,
) {
    let lobby_id = trigger.lobby_id;

    for mut server in &mut servers {
        let players = lobby_players
            .iter()
            .filter(|(member, _, _)| member.lobby_id == lobby_id)
            .map(|(_, player, credentials)| (player.id, credentials.clone()))
            .collect();

        let request = CreateGameRequest {
            lobby_id,
            players,
            courses: vec!["0002".to_owned(), "0002".to_owned()],
        };

        let message: String = ServerPacket::CreateGame(request).into();

        info!("Sending message {:?}", message);
        server.send.push(Bytes::from_owner(message));

        break;
    }
}

#[derive(Debug, Event)]
pub(crate) struct GameStarted {
    pub(crate) lobby_id: LobbyId,
    pub(crate) server: String,
}
