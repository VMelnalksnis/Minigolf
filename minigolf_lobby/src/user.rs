use {
    crate::{Args, Lobby},
    aeronet::{io::Session, io::bytes::Bytes, io::connection::LocalAddr, io::server::Server},
    aeronet_websocket::server::{ServerConfig, WebSocketServer},
    bevy::prelude::*,
    minigolf::lobby::{UserClientPacket, UserServerPacket},
    std::ops::RangeFull,
};

#[derive(Debug)]
pub(super) struct UserPlugin;

impl Plugin for UserPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, open_listener)
            .add_observer(on_opened)
            .add_observer(on_connected)
            .add_systems(FixedUpdate, handle_messages);
    }
}

#[derive(Debug, Component)]
struct UserListener;

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
    servers: Query<&Parent>,
    users: Query<&UserListener>,
) {
    let client = trigger.entity();
    let server = servers
        .get(client)
        .expect("connected session should have a server")
        .get();

    if let Ok(_) = users.get(server) {
        info!("User {client} connected to {server}");
    }
}

fn handle_messages(
    mut sessions: Query<(Entity, &mut Session, &Parent)>,
    users: Query<&UserListener>,
    mut commands: Commands,
) {
    for (entity, mut session, parent) in &mut sessions {
        let Ok(_) = users.get(parent.get()) else {
            continue;
        };

        let session = &mut *session;
        for message in session.recv.drain(RangeFull::default()) {
            let client_packet = UserClientPacket::from(message.payload.as_ref());
            info!("{client_packet:?}");

            match client_packet {
                UserClientPacket::Hello => {
                    let response: String = UserServerPacket::Hello.into();
                    session.send.push(Bytes::from_owner(response));
                }

                UserClientPacket::CreateLobby => {
                    let lobby = commands.spawn(Lobby { owner: entity }).id();
                    let lobby_name = format!("{}", lobby);
                    let response: String = UserServerPacket::LobbyCreated(&lobby_name).into();
                    session.send.push(Bytes::from_owner(response));
                }

                UserClientPacket::JoinLobby(_) => {
                    todo!()
                }
            };
        }
    }
}
