use {
    crate::server::Args,
    aeronet_replicon::server::AeronetRepliconServer,
    aeronet_websocket::server::{WebSocketServer, WebSocketServerPlugin},
    aeronet_webtransport::{
        cert,
        server::{WebTransportServer, WebTransportServerPlugin},
        wtransport,
    },
    bevy::prelude::*,
    core::time::Duration,
};

/// Sets up minigolf server listeners
pub(crate) struct ServerListenerPlugin;

impl Plugin for ServerListenerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((WebTransportServerPlugin, WebSocketServerPlugin));
        app.add_systems(Startup, (open_web_transport_server, open_web_socket_server));
    }
}

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

    let server_configuration = aeronet_webtransport::server::ServerConfig::builder()
        .with_bind_default(args.web_transport_port)
        .with_identity(identity)
        .keep_alive_interval(Some(Duration::from_secs(1)))
        .max_idle_timeout(Some(Duration::from_secs(5)))
        .expect("should be a valid idle timeout")
        .build();

    let server = commands
        .spawn((Name::new("WebTransport Server"), AeronetRepliconServer))
        .queue(WebTransportServer::open(server_configuration))
        .id();

    info!("Opening WebTransport server {server}");
}

fn open_web_socket_server(mut commands: Commands, args: Res<Args>) {
    let server_configuration = aeronet_websocket::server::ServerConfig::builder()
        .with_bind_default(args.web_socket_port)
        .with_no_encryption();

    let server = commands
        .spawn((Name::new("WebSocket Server"), AeronetRepliconServer))
        .queue(WebSocketServer::open(server_configuration))
        .id();

    info!("Opening WebSocket server {server}");
}
