use aeronet::io::bytes::Bytes;
use aeronet::transport::AeronetTransportPlugin;
use aeronet_websocket::client::WebSocketClientPlugin;
use minigolf::lobby::{GameClientPacket, GameServerPacket};
use std::ops::RangeFull;
use {
    crate::server::Args,
    aeronet::io::{
        Session,
        connection::{DisconnectReason, Disconnected, LocalAddr},
        server::Server,
    },
    aeronet_replicon::server::{AeronetRepliconServer, AeronetRepliconServerPlugin},
    aeronet_websocket::{
        client::WebSocketClient,
        server::{WebSocketServer, WebSocketServerPlugin},
    },
    aeronet_webtransport::{
        cert,
        server::{SessionRequest, SessionResponse, WebTransportServer, WebTransportServerPlugin},
        wtransport,
    },
    bevy::prelude::*,
    bevy_replicon::prelude::*,
    core::time::Duration,
};

/// Sets up minigolf server networking.
#[derive(Debug)]
pub(crate) struct ServerNetworkPlugin;

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((WebTransportServerPlugin, WebSocketServerPlugin))
            .add_plugins((WebSocketClientPlugin, AeronetTransportPlugin))
            .add_plugins((
                RepliconPlugins.set(ServerPlugin {
                    // 1 frame lasts `1.0 / TICK_RATE` anyway
                    tick_policy: TickPolicy::Manual,
                    ..Default::default()
                }),
                AeronetRepliconServerPlugin,
            ))
            .add_observer(on_opened)
            .add_observer(on_session_request)
            .add_observer(on_connected)
            .add_observer(on_disconnected)
            .add_systems(
                Startup,
                (
                    open_web_transport_server,
                    open_web_socket_server,
                    connect_to_lobby,
                ),
            )
            .add_systems(FixedUpdate, recv_lobby_messages);
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

fn on_opened(trigger: Trigger<OnAdd, Server>, servers: Query<&LocalAddr>) {
    let server = trigger.entity();
    let local_addr = servers
        .get(server)
        .expect("opened server should have a binding socket `LocalAddr`");
    info!("{server} opened on {}", **local_addr);
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

fn on_connected(
    trigger: Trigger<OnAdd, Session>,
    servers: Query<&Parent>,
    names: Query<&Name>,
    mut sessions: Query<&mut Session>,
) {
    let client = trigger.entity();

    if let Ok(server) = servers.get(client).map(Parent::get) {
        info!("{client} connected to {server}");
    } else if let Ok(name) = names.get(client) {
        info!("Connected to {name}");
        let mut session = sessions.get_mut(client).unwrap();

        let message: String = GameClientPacket::Hello.into();
        session.send.push(Bytes::from_owner(message));
    } else {
        return;
    };
}

fn recv_lobby_messages(
    mut sessions: Query<&mut Session, With<WebSocketClient>>,
    servers: Query<&LocalAddr, With<WebSocketServer>>,
) {
    for mut session in &mut sessions {
        let session = &mut *session;

        for message in session.recv.drain(RangeFull::default()) {
            let server_packet = GameServerPacket::from(message.payload.as_ref());
            info!("{server_packet:?}");

            match server_packet {
                GameServerPacket::Hello => {
                    let server_address = servers.get_single().unwrap().0;
                    let response: String = GameClientPacket::Available(server_address).into();
                    session.send.push(Bytes::from_owner(response));
                }
            }
        }
    }
}

fn on_disconnected(trigger: Trigger<Disconnected>, servers: Query<&Parent>, names: Query<&Name>) {
    let client = trigger.entity();

    if let Ok(server) = servers.get(client).map(Parent::get) {
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
    } else if let Ok(name) = names.get(client) {
        match &trigger.reason {
            DisconnectReason::User(reason) => {
                info!("Disconnected from {name} by user: {reason}");
            }
            DisconnectReason::Peer(reason) => {
                info!("Disconnected from {name} by peer: {reason}");
            }
            DisconnectReason::Error(err) => {
                warn!("Disconnected from {name} due to error: {err:?}");
            }
        }

        info!("Disconnected from {name}");
    } else {
        return;
    };
}
