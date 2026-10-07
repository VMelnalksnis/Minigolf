use {
    crate::{Args, output},
    aeronet::io::{
        Session,
        bytes::Bytes,
        connection::{DisconnectReason, Disconnected},
    },
    aeronet_replicon::client::{AeronetRepliconClient, AeronetRepliconClientPlugin},
    aeronet_websocket::client::{ClientConfig, WebSocketClient, WebSocketClientPlugin},
    bevy::prelude::*,
    bevy_replicon::prelude::*,
    minigolf::{
        AuthenticatePlayer, GameState, PlayerCredentials, RequestAuthentication,
        lobby::{
            LobbyId, PlayerId,
            user::{ClientPacket, ServerPacket},
        },
    },
};

/// Connects to the lobby server, and to the game server once a game is started.
pub(crate) struct CliNetworkPlugin;

impl Plugin for CliNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WebSocketClientPlugin)
            .add_plugins((RepliconPlugins, AeronetRepliconClientPlugin));

        app.init_resource::<CurrentLobby>();

        app.add_observer(on_connected);
        app.add_observer(on_disconnected);

        app.add_systems(Startup, connect_to_lobby_server);
        app.add_systems(Update, (handle_lobby_packets, authenticate));
    }
}

/// Marker for the session with the lobby server.
#[derive(Component, Debug)]
pub(crate) struct LobbySession;

/// Player id and credentials given by the lobby server.
#[derive(Resource, Clone, Debug)]
pub(crate) struct Authentication {
    pub(crate) id: PlayerId,
    credentials: PlayerCredentials,
}

/// The lobby the player is in, according to the lobby server.
#[derive(Resource, Default, Debug)]
pub(crate) struct CurrentLobby(pub(crate) Option<LobbyId>);

fn connect_to_lobby_server(args: Res<Args>, mut commands: Commands) {
    output!("lobby connecting {}", args.lobby);

    commands
        .spawn((Name::new("Lobby server"), LobbySession))
        .queue(WebSocketClient::connect(
            ClientConfig::builder().with_no_encryption(),
            args.lobby.clone(),
        ));
}

/// Sends a packet to the lobby server, returns whether it is connected.
pub(crate) fn send_to_lobby(
    lobby: &mut Query<&mut Session, With<LobbySession>>,
    packet: ClientPacket,
) -> bool {
    let Ok(mut session) = lobby.single_mut() else {
        return false;
    };

    let packet: String = packet.into();
    session.send.push(Bytes::from(packet));
    true
}

fn handle_lobby_packets(
    mut lobby: Query<&mut Session, With<LobbySession>>,
    mut current_lobby: ResMut<CurrentLobby>,
    mut commands: Commands,
) {
    let Ok(mut session) = lobby.single_mut() else {
        return;
    };

    for packet in session.recv.drain(..) {
        match ServerPacket::from(packet.payload.as_ref()) {
            ServerPacket::Hello(id, credentials) => {
                output!("lobby hello player={id:?}");
                commands.insert_resource(Authentication { id, credentials });
            }

            ServerPacket::LobbyCreated(lobby_id) => {
                output!("lobby created {lobby_id}");
                current_lobby.0 = Some(lobby_id);
            }

            ServerPacket::AvailableLobbies(lobby_ids) => {
                output!("lobby list {lobby_ids:?}");
            }

            ServerPacket::LobbyJoined(lobby_id, player_ids) => {
                output!("lobby joined {lobby_id} players={player_ids:?}");
                current_lobby.0 = Some(lobby_id);
            }

            ServerPacket::PlayerJoined(player) => {
                output!("lobby player-joined {:?}", player.player_id);
            }

            ServerPacket::PlayerLeft(player) => {
                output!("lobby player-left {:?}", player.player_id);
            }

            ServerPacket::GameStarted(server) => {
                output!("game connecting {server}");

                commands
                    .spawn((Name::new("Game server"), AeronetRepliconClient))
                    .queue(WebSocketClient::connect(
                        ClientConfig::builder().with_no_encryption(),
                        server,
                    ));
            }
        }
    }
}

fn authenticate(
    mut reader: MessageReader<RequestAuthentication>,
    authentication: Option<Res<Authentication>>,
    mut writer: MessageWriter<AuthenticatePlayer>,
) {
    for _ in reader.read() {
        let Some(authentication) = &authentication else {
            output!("error authentication requested before the lobby server said hello");
            continue;
        };

        output!("game authenticating");
        writer.write(AuthenticatePlayer {
            id: authentication.id,
            credentials: authentication.credentials.clone(),
        });
    }
}

fn on_connected(
    add: On<Add, Session>,
    lobby: Query<(), With<LobbySession>>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    if lobby.contains(add.entity) {
        output!("lobby connected");
    } else {
        output!("game connected");
        game_state.set(GameState::Playing);
    }
}

fn on_disconnected(
    disconnected: On<Disconnected>,
    lobby: Query<(), With<LobbySession>>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let reason = match &disconnected.event().reason {
        DisconnectReason::ByUser(reason) => format!("by user: {reason}"),
        DisconnectReason::ByPeer(reason) => format!("by peer: {reason}"),
        DisconnectReason::ByError(error) => format!("by error: {error:?}"),
    };

    if lobby.contains(disconnected.entity) {
        output!("lobby disconnected {reason}");
    } else {
        output!("game disconnected {reason}");
        game_state.set(GameState::None);
    }
}
