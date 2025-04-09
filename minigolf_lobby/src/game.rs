use {
    crate::{Args, LobbyMember},
    aeronet::{io::Session, io::bytes::Bytes, io::connection::LocalAddr, io::server::Server},
    aeronet_websocket::server::{ServerConfig, WebSocketServer},
    bevy::prelude::*,
    minigolf::{
        lobby::LobbyId,
        lobby::{GameClientPacket, GameServerPacket},
    },
    std::{net::SocketAddr, ops::RangeFull},
};

#[derive(Debug)]
pub(super) struct GameServerPlugin;

impl Plugin for GameServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, open_listener)
            .add_observer(on_opened)
            .add_observer(on_connected)
            .add_observer(on_game_server_added)
            .add_observer(on_game_server_removed)
            .add_systems(FixedUpdate, handle_messages)
            .add_event::<StartGame>()
            .add_systems(FixedUpdate, start_game)
            .add_event::<GameStarted>();
    }
}

#[derive(Debug, Component)]
struct GamerServerListener;

#[derive(Debug, Component)]
struct GameServerSession;

#[derive(Debug, Component)]
struct GameServer {
    address: SocketAddr,
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
    let server = trigger.entity();
    let local_addr = addresses
        .get(server)
        .expect("opened server should have a binding socket `LocalAddr`");

    if let Ok(_) = games.get(server) {
        info!("{server} opened on {} for game servers", **local_addr);
    }
}

fn on_connected(
    trigger: Trigger<OnAdd, Session>,
    servers: Query<&Parent>,
    games: Query<&GamerServerListener>,
    mut commands: Commands,
) {
    let client = trigger.entity();
    let server = servers
        .get(client)
        .expect("connected session should have a server")
        .get();

    if let Ok(_) = games.get(server) {
        info!("Game server {client} connected to {server}");
        commands.entity(client).insert(GameServerSession);
    }
}

fn handle_messages(
    mut sessions: Query<(Entity, &mut Session), With<GameServerSession>>,
    mut game_started_writer: EventWriter<GameStarted>,
    mut commands: Commands,
) {
    for (entity, mut session) in &mut sessions {
        let session = &mut *session;

        for message in session.recv.drain(RangeFull::default()) {
            let client_packet = GameClientPacket::from(message.payload.as_ref());
            info!("{client_packet:?}");

            match client_packet {
                GameClientPacket::Hello => {
                    let response: String = GameServerPacket::Hello.into();
                    session.send.push(Bytes::from_owner(response));
                }

                GameClientPacket::Available(game_server_address) => {
                    commands.entity(entity).insert(GameServer {
                        address: game_server_address,
                    });
                }

                GameClientPacket::Busy => {
                    todo!()
                }

                GameClientPacket::GameCreated(lobby_id) => {
                    game_started_writer.send(GameStarted {
                        lobby_id,
                        server: "ws://localhost:25566".into(),
                    });
                }
            }

            if let GameClientPacket::Available(_) = client_packet {
                commands.entity(entity).remove::<GameServer>();
            }
        }
    }
}

fn on_game_server_added(trigger: Trigger<OnAdd, GameServer>, servers: Query<&GameServer>) {
    let entity = trigger.entity();
    let connected_server = servers.get(entity).unwrap();
    info!("Added new game server {connected_server:?}");

    for server in &servers {
        info!("Available server {server:?}");
    }
}

fn on_game_server_removed(_trigger: Trigger<OnRemove, GameServer>, servers: Query<&GameServer>) {
    info!("Removed game server");

    for server in &servers {
        info!("Available server {server:?}");
    }
}

#[derive(Debug, Event)]
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

fn start_game(
    mut start_game_reader: EventReader<StartGame>,
    mut servers: Query<&mut Session, With<GameServer>>,
) {
    for start_game in start_game_reader.read() {
        for mut server in &mut servers {
            let message: String = GameServerPacket::CreateGame(start_game.lobby_id).into();
            server.send.push(Bytes::from_owner(message));

            break;
        }
    }
}

#[derive(Debug, Event)]
pub(crate) struct GameStarted {
    pub(crate) lobby_id: LobbyId,
    pub(crate) server: String,
}
