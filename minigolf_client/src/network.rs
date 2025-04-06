use {
    aeronet_replicon::client::AeronetRepliconClientPlugin,
    aeronet_websocket::client::WebSocketClientPlugin,
    aeronet_webtransport::{cert, client::WebTransportClientPlugin},
    bevy::prelude::*,
    bevy_replicon::prelude::*,
};

/// Sets up minigolf client networking.
#[derive(Debug)]
pub(crate) struct ClientNetworkPlugin;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((WebTransportClientPlugin, WebSocketClientPlugin))
            .add_plugins((RepliconPlugins, AeronetRepliconClientPlugin));
    }
}

//
// WebTransport
//

type WebTransportClientConfig = aeronet_webtransport::client::ClientConfig;

#[cfg(target_family = "wasm")]
pub(crate) fn web_transport_config(cert_hash: String) -> WebTransportClientConfig {
    use aeronet_webtransport::xwt_web::{CertificateHash, HashAlgorithm};

    let server_certificate_hashes = match cert::hash_from_b64(&cert_hash) {
        Ok(hash) => vec![CertificateHash {
            algorithm: HashAlgorithm::Sha256,
            value: Vec::from(hash),
        }],
        Err(err) => {
            warn!("Failed to read certificate hash from string: {err:?}");
            Vec::new()
        }
    };

    WebTransportClientConfig {
        server_certificate_hashes,
        ..Default::default()
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn web_transport_config(cert_hash: String) -> WebTransportClientConfig {
    use {aeronet_webtransport::wtransport::tls::Sha256Digest, core::time::Duration};

    let config = WebTransportClientConfig::builder().with_bind_default();

    let config = if cert_hash.is_empty() {
        warn!("Connecting without certificate validation");
        config.with_no_cert_validation()
    } else {
        match cert::hash_from_b64(&cert_hash) {
            Ok(hash) => config.with_server_certificate_hashes([Sha256Digest::new(hash)]),
            Err(err) => {
                warn!("Failed to read certificate hash from string: {err:?}");
                config.with_server_certificate_hashes([])
            }
        }
    };

    config
        .keep_alive_interval(Some(Duration::from_secs(1)))
        .max_idle_timeout(Some(Duration::from_secs(5)))
        .expect("should be a valid idle timeout")
        .build()
}

//
// WebSocket
//

type WebSocketClientConfig = aeronet_websocket::client::ClientConfig;

#[cfg(target_family = "wasm")]
pub(crate) fn web_socket_config() -> WebSocketClientConfig {
    WebSocketClientConfig::default()
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn web_socket_config() -> WebSocketClientConfig {
    WebSocketClientConfig::builder().with_no_cert_validation()
}
