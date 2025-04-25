use {
    crate::ui::{ServerState, lobby::LobbyUi, lobby_server::LobbyServerSession},
    aeronet::io::{Session, SessionEndpoint, connection::Disconnected},
    aeronet_replicon::client::{AeronetRepliconClient, AeronetRepliconClientPlugin},
    aeronet_websocket::client::{WebSocketClient, WebSocketClientPlugin},
    aeronet_webtransport::{
        cert,
        client::{WebTransportClient, WebTransportClientPlugin},
    },
    bevy::prelude::*,
    bevy_replicon::prelude::*,
    minigolf::{
        AuthenticatePlayer, PlayerCredentials, RequestAuthentication,
        lobby::{PlayerId, user::ServerPacket},
    },
};

/// Sets up minigolf client networking.
#[derive(Debug)]
pub(crate) struct ClientNetworkPlugin;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((WebTransportClientPlugin, WebSocketClientPlugin))
            .add_plugins((RepliconPlugins, AeronetRepliconClientPlugin));

        app.add_observer(on_connecting);
        app.add_observer(on_disconnected);

        app.add_systems(
            Update,
            (handle_lobby_server_packets, on_authentication_requested),
        );
    }
}

#[cfg(target_family = "wasm")]
pub(crate) fn web_transport_config(
    cert_hash: String,
) -> aeronet_webtransport::client::ClientConfig {
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

    aeronet_webtransport::client::ClientConfig {
        server_certificate_hashes,
        ..Default::default()
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn web_transport_config(
    cert_hash: String,
) -> aeronet_webtransport::client::ClientConfig {
    use {aeronet_webtransport::wtransport::tls::Sha256Digest, core::time::Duration};

    let config = aeronet_webtransport::client::ClientConfig::builder().with_bind_default();

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

pub(crate) fn connect_to_lobby_server(target: &str, mut commands: Commands) {
    #[cfg(target_family = "wasm")]
    let config = aeronet_websocket::client::ClientConfig::default();

    #[cfg(not(target_family = "wasm"))]
    let config = aeronet_websocket::client::ClientConfig::builder().with_no_cert_validation();

    commands
        .spawn((
            Name::new(format!("Lobby server {target}")),
            LobbyServerSession,
        ))
        .queue(WebSocketClient::connect(config, target));
}

fn on_connecting(trigger: Trigger<OnAdd, SessionEndpoint>, names: Query<&Name>) {
    let entity = trigger.target();
    let name = names
        .get(entity)
        .expect("our session entity should have a name");

    info!("{name} connecting");
}

fn on_disconnected(
    trigger: Trigger<Disconnected>,
    names: Query<&Name>,
    game_servers: Query<(), With<AeronetRepliconClient>>,
    mut state: ResMut<NextState<ServerState>>,
) {
    let session = trigger.target();
    let name = names
        .get(session)
        .expect("our session entity should have a name");

    match trigger.event() {
        Disconnected::ByUser(reason) => {
            info!("{name} disconnected by user: {reason}");
        }
        Disconnected::ByPeer(reason) => {
            info!("{name} disconnected by peer: {reason}");
        }
        Disconnected::ByError(err) => {
            info!("{name} disconnected due to error: {err:?}");
        }
    };

    if let Ok(_) = game_servers.get(session) {
        info!("Disconnected from game server, falling back to current lobby");
        state.set(ServerState::Lobby);
    }
}

fn handle_lobby_server_packets(
    mut sessions: Query<&mut Session, With<LobbyServerSession>>,
    mut server_state: ResMut<NextState<ServerState>>,
    mut lobby_ui: ResMut<LobbyUi>,
    mut commands: Commands,
) {
    let Ok(mut lobby_session) = sessions.single_mut() else {
        return;
    };

    for received_packet in lobby_session.recv.drain(..) {
        let packet = ServerPacket::from(received_packet.payload.as_ref());
        info!("Lobby packet received: {:?}", packet);

        match packet {
            ServerPacket::Hello(id, credentials) => {
                commands.insert_resource(Authentication::new(id, credentials));
            }

            ServerPacket::LobbyCreated(lobby_id) => {
                server_state.set(ServerState::Lobby);

                commands.insert_resource::<LobbyUi>(LobbyUi::new_lobby(lobby_id.to_string()));
            }

            ServerPacket::AvailableLobbies(_) => {}

            ServerPacket::LobbyJoined(lobby_id, player_ids) => {
                server_state.set(ServerState::Lobby);

                let ui = LobbyUi::new_existing_lobby(lobby_id.to_string(), player_ids);
                commands.insert_resource::<LobbyUi>(ui);
            }

            ServerPacket::GameStarted(server) => {
                server_state.set(ServerState::GameServer);

                #[cfg(target_family = "wasm")]
                let config = aeronet_websocket::client::ClientConfig::default();

                #[cfg(not(target_family = "wasm"))]
                let config =
                    aeronet_websocket::client::ClientConfig::builder().with_no_cert_validation();
                commands
                    .spawn((
                        Name::new(format!("Game server {server}")),
                        AeronetRepliconClient,
                    ))
                    .queue(WebSocketClient::connect(config, server));
            }

            ServerPacket::PlayerJoined(player) => {
                lobby_ui.add_player(player.player_id);
            }

            ServerPacket::PlayerLeft(player) => {
                lobby_ui.remove_player(player.player_id);
            }
        }
    }
}

#[derive(Resource, Reflect, Clone, Debug)]
pub(crate) struct Authentication {
    pub(crate) id: PlayerId,
    credentials: PlayerCredentials,
}

impl Authentication {
    pub(crate) fn new(id: PlayerId, credentials: PlayerCredentials) -> Self {
        Authentication { id, credentials }
    }
}

fn on_authentication_requested(
    mut reader: EventReader<RequestAuthentication>,
    authentication: Option<Res<Authentication>>,
    mut writer: EventWriter<AuthenticatePlayer>,
) {
    for _ in reader.read() {
        let auth = match &authentication {
            None => Authentication::new(PlayerId::new(), PlayerCredentials::default()),
            Some(res) => Authentication::new(res.id, res.credentials.clone()),
        };

        info!("Sending {:?}", auth);
        writer.write(AuthenticatePlayer {
            id: auth.id,
            credentials: auth.credentials,
        });
    }
}
