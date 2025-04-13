use {
    crate::network::web_socket_config,
    aeronet::io::{
        Session, SessionEndpoint,
        bytes::Bytes,
        connection::{Disconnect, DisconnectReason, Disconnected},
    },
    aeronet_replicon::client::AeronetRepliconClient,
    aeronet_websocket::client::WebSocketClient,
    bevy::{input::common_conditions::input_toggle_active, prelude::*},
    bevy_egui::{EguiContexts, EguiPlugin, egui},
    bevy_inspector_egui::quick::WorldInspectorPlugin,
    bevy_replicon::prelude::*,
    iyes_perf_ui::prelude::*,
    minigolf::{
        AuthenticatePlayer, PlayerCredentials, RequestAuthentication,
        lobby::{LobbyId, PlayerId, UserClientPacket, UserServerPacket},
    },
};

/// Sets up minigolf client UI.
#[derive(Debug)]
pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GlobalUi>()
            .register_type::<LobbyServerUi>()
            .register_type::<ServerState>()
            .init_state::<ServerState>()
            .add_plugins(EguiPlugin)
            .add_plugins(
                WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
            )
            .init_resource::<GlobalUi>()
            .init_resource::<LobbyServerUi>()
            .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
            .add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin)
            .add_plugins(bevy::diagnostic::SystemInformationDiagnosticsPlugin)
            .add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin)
            .add_plugins(PerfUiPlugin)
            .add_systems(Startup, enable_perf)
            .add_systems(
                OnEnter(ServerState::LobbyServer),
                connect_to_default_lobby_server,
            )
            .add_systems(Update, lobby_server_ui.in_set(LobbyServerUiSet))
            .configure_sets(
                Update,
                LobbyServerUiSet.run_if(in_state(ServerState::LobbyServer)),
            )
            .add_systems(Update, handle_lobby_server_packets)
            .add_systems(Update, on_authentication_requested)
            .add_systems(Update, global_ui)
            .add_observer(on_connecting)
            .add_observer(on_connected_to_lobby_server)
            .add_observer(on_disconnected)
            .init_resource::<LobbiesUi>()
            .configure_sets(Update, LobbiesUiSet.run_if(in_state(ServerState::Lobbies)))
            .add_systems(Update, lobbies_ui.in_set(LobbiesUiSet));

        app.init_resource::<LobbyUi>()
            .configure_sets(Update, LobbyUiSet.run_if(in_state(ServerState::Lobby)))
            .add_systems(Update, lobby_ui.in_set(LobbyUiSet));
    }
}

fn enable_perf(mut commands: Commands) {
    commands.spawn(PerfUiDefaultEntries::default());
}

#[derive(States, Reflect, Default, Debug, Clone, PartialEq, Eq, Hash)]
enum ServerState {
    #[default]
    LobbyServer,
    Lobbies,
    Lobby,
    GameServer,
}

/// Systems for selecting and connecting to a lobby server
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct LobbyServerUiSet;

#[derive(Component, Reflect, Debug)]
struct TempMarker;

fn handle_lobby_server_packets(
    mut sessions: Query<&mut Session, With<LobbyServerSession>>,
    mut server_state: ResMut<NextState<ServerState>>,
    mut commands: Commands,
) {
    let Ok(mut lobby_session) = sessions.get_single_mut() else {
        return;
    };

    for received_packet in lobby_session.recv.drain(..) {
        let packet = UserServerPacket::from(received_packet.payload.as_ref());
        info!("Lobby packet received: {:?}", packet);

        match packet {
            UserServerPacket::Hello(id, credentials) => {
                commands.insert_resource(Authentication { id, credentials });
            }
            UserServerPacket::LobbyCreated(_) => {
                server_state.set(ServerState::Lobby);
            }
            UserServerPacket::AvailableLobbies(_) => {}
            UserServerPacket::LobbyJoined(_) => {
                server_state.set(ServerState::Lobby);
            }
            UserServerPacket::GameStarted(server) => {
                server_state.set(ServerState::GameServer);

                let config = web_socket_config();
                let name = format!("Game server {server}");

                commands
                    .spawn((Name::new(name), AeronetRepliconClient))
                    .queue(WebSocketClient::connect(config, server));
            }
            UserServerPacket::PlayerJoined(_) => {}
            UserServerPacket::PlayerLeft(_) => {}
        }
    }
}

fn on_authentication_requested(
    authentication: Option<Res<Authentication>>,
    mut writer: EventWriter<AuthenticatePlayer>,
    mut readed: EventReader<RequestAuthentication>,
) {
    for _ in readed.read() {
        let auth = match &authentication {
            None => Authentication {
                id: PlayerId::new(),
                credentials: PlayerCredentials::default(),
            },
            Some(res) => Authentication {
                id: res.id,
                credentials: res.credentials.clone(),
            },
        };

        info!("Sending {:?}", auth);
        writer.send(AuthenticatePlayer {
            id: auth.id,
            credentials: auth.credentials,
        });
    }
}

#[derive(Resource, Reflect, Debug, Default)]
struct LobbyServerUi {
    target: String,
}

#[derive(Resource, Reflect, Clone, Debug)]
pub(crate) struct Authentication {
    pub(crate) id: PlayerId,
    credentials: PlayerCredentials,
}

#[derive(Debug, Component)]
struct LobbyServerSession;

const DEFAULT_LOBBY_TARGET: &str = "ws://localhost:25567";

fn connect_to_default_lobby_server(mut global_ui: ResMut<GlobalUi>, mut commands: Commands) {
    let target = DEFAULT_LOBBY_TARGET;
    let config = web_socket_config();

    global_ui.session_id += 1;
    let name = format!("Lobby server {}. {target}", global_ui.session_id);

    commands
        .spawn((Name::new(name), LobbyServerSession))
        .queue(WebSocketClient::connect(config, target));
}

fn lobby_server_ui(
    mut commands: Commands,
    mut egui: EguiContexts,
    mut global_ui: ResMut<GlobalUi>,
    mut ui_state: ResMut<LobbyServerUi>,
) {
    egui::Window::new("Select lobby server").show(egui.ctx_mut(), |ui| {
        let enter_pressed = ui.input(|state| state.key_pressed(egui::Key::Enter));

        let mut connect = false;
        ui.horizontal(|ui| {
            let connect_resp = ui.add(
                egui::TextEdit::singleline(&mut ui_state.target)
                    .hint_text(format!("{DEFAULT_LOBBY_TARGET} | [enter] to connect")),
            );
            connect |= connect_resp.lost_focus() && enter_pressed;
            connect |= ui.button("Connect").clicked();
        });

        if connect {
            let mut target = ui_state.target.clone();
            if target.is_empty() {
                DEFAULT_LOBBY_TARGET.clone_into(&mut target);
            }

            let config = web_socket_config();

            global_ui.session_id += 1;
            let name = format!("{}. {target}", global_ui.session_id);
            commands
                .spawn((Name::new(name), LobbyServerSession))
                .queue(WebSocketClient::connect(config, target));
        }
    });
}

fn on_connected_to_lobby_server(
    trigger: Trigger<OnAdd, Session>,
    lobby_servers: Query<(&Session, &Name), With<LobbyServerSession>>,
    mut next_state: ResMut<NextState<ServerState>>,
) {
    let entity = trigger.entity();
    let Ok((_session, name)) = lobby_servers.get(entity) else {
        return;
    };

    info!("{name} connected");
    next_state.set(ServerState::Lobbies);
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct LobbiesUiSet;

#[derive(Resource, Reflect, Debug, Default)]
struct LobbiesUi {
    lobby_id: String,
}

fn lobbies_ui(
    mut egui: EguiContexts,
    mut lobbies_ui: ResMut<LobbiesUi>,
    mut lobby_session: Query<&mut Session, With<LobbyServerSession>>,
) {
    egui::Window::new("Select lobby").show(egui.ctx_mut(), |ui| {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut lobbies_ui.lobby_id);

            if ui.button("Join lobby").clicked() {
                let Ok(id) = lobbies_ui.lobby_id.parse::<LobbyId>() else {
                    lobbies_ui.lobby_id = String::new();
                    return;
                };

                info!("Joining lobby {}", lobbies_ui.lobby_id);

                let mut session = lobby_session.single_mut();
                let request: String = UserClientPacket::JoinLobby(id).into();
                session.send.push(Bytes::from(request));
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Create lobby").clicked() {
                info!("Creating lobby");

                let mut session = lobby_session.single_mut();
                let request: String = UserClientPacket::CreateLobby.into();
                session.send.push(Bytes::from(request));
            }
        })
    });
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct LobbyUiSet;

#[derive(Resource, Reflect, Debug, Default)]
struct LobbyUi {
    lobby_id: String,
    players: Vec<PlayerId>,
}

fn lobby_ui(
    mut egui: EguiContexts,
    lobby_ui: ResMut<LobbyUi>,
    mut lobby_session: Query<&mut Session, With<LobbyServerSession>>,
    mut state: ResMut<NextState<ServerState>>,
) {
    let title = format!("Lobby {}", lobby_ui.lobby_id);

    egui::Window::new(title).show(egui.ctx_mut(), |ui| {
        ui.horizontal(|ui| {
            if ui.button("Start game").clicked() {
                info!("Starting game");

                let mut session = lobby_session.single_mut();
                let request: String = UserClientPacket::StartGame.into();
                session.send.push(Bytes::from(request));
            }

            if ui.button("Leave lobby").clicked() {
                info!("Leaving lobby");

                let mut session = lobby_session.single_mut();
                let request: String = UserClientPacket::LeaveLobby.into();
                session.send.push(Bytes::from(request));
                state.set(ServerState::Lobbies);
            }
        })
    });
}

#[derive(Debug, Default, Resource, Reflect)]
struct GlobalUi {
    session_id: usize,
}

fn on_connecting(trigger: Trigger<OnAdd, SessionEndpoint>, names: Query<&Name>) {
    let entity = trigger.entity();
    let name = names
        .get(entity)
        .expect("our session entity should have a name");

    info!("{name} connecting");
}

fn on_disconnected(trigger: Trigger<Disconnected>, names: Query<&Name>) {
    let session = trigger.entity();
    let name = names
        .get(session)
        .expect("our session entity should have a name");

    match &trigger.reason {
        DisconnectReason::User(reason) => {
            info!("{name} disconnected by user: {reason}");
        }
        DisconnectReason::Peer(reason) => {
            info!("{name} disconnected by peer: {reason}");
        }
        DisconnectReason::Error(err) => {
            info!("{name} disconnected due to error: {err:?}");
        }
    };
}

fn global_ui(
    mut commands: Commands,
    mut egui: EguiContexts,
    sessions: Query<(Entity, &Name, Option<&Session>), With<SessionEndpoint>>,
    replicon_client: Res<RepliconClient>,
) {
    let stats = replicon_client.stats();
    egui::Window::new("Session Log").show(egui.ctx_mut(), |ui| {
        ui.label("Replicon reports:");
        ui.horizontal(|ui| {
            ui.label(match replicon_client.status() {
                RepliconClientStatus::Disconnected => "Disconnected",
                RepliconClientStatus::Connecting => "Connecting",
                RepliconClientStatus::Connected { .. } => "Connected",
            });
            ui.separator();

            ui.label(format!("RTT {:.0}ms", stats.rtt * 1000.0));
            ui.separator();

            ui.label(format!("Pkt Loss {:.1}%", stats.packet_loss * 100.0));
            ui.separator();

            ui.label(format!("Rx {:.0}bps", stats.received_bps));
            ui.separator();

            ui.label(format!("Tx {:.0}bps", stats.sent_bps));
        });

        for (session, name, connected) in &sessions {
            ui.horizontal(|ui| {
                if connected.is_some() {
                    ui.label(format!("{name} connected"));
                } else {
                    ui.label(format!("{name} connecting"));
                }

                if ui.button("Disconnect").clicked() {
                    commands.trigger_targets(Disconnect::new("disconnected by user"), session);
                }
            });
        }
    });
}
