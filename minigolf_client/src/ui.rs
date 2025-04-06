use {
    crate::network::{web_socket_config, web_transport_config},
    aeronet::io::{
        Session, SessionEndpoint,
        connection::{Disconnect, DisconnectReason, Disconnected},
    },
    aeronet_replicon::client::AeronetRepliconClient,
    aeronet_websocket::client::WebSocketClient,
    aeronet_webtransport::client::WebTransportClient,
    bevy::{
        ecs::query::QuerySingleError, input::common_conditions::input_toggle_active, prelude::*,
    },
    bevy_egui::{EguiContexts, EguiPlugin, egui},
    bevy_inspector_egui::quick::WorldInspectorPlugin,
    bevy_replicon::prelude::*,
};

/// Sets up minigolf client UI.
#[derive(Debug)]
pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .add_plugins(
                WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
            )
            .init_resource::<GlobalUi>()
            .init_resource::<WebTransportUi>()
            .init_resource::<WebSocketUi>()
            .add_systems(Update, (web_transport_ui, web_socket_ui, global_ui).chain())
            .add_observer(on_connecting)
            .add_observer(on_connected)
            .add_observer(on_disconnected);
    }
}

#[derive(Debug, Default, Resource)]
struct GlobalUi {
    session_id: usize,
    log: Vec<String>,
}

#[derive(Debug, Default, Resource)]
struct WebTransportUi {
    target: String,
    cert_hash: String,
}

#[derive(Debug, Default, Resource)]
struct WebSocketUi {
    target: String,
}

fn on_connecting(
    trigger: Trigger<OnAdd, SessionEndpoint>,
    names: Query<&Name>,
    mut ui_state: ResMut<GlobalUi>,
) {
    let entity = trigger.entity();
    let name = names
        .get(entity)
        .expect("our session entity should have a name");
    ui_state.log.push(format!("{name} connecting"));
}

fn on_connected(
    trigger: Trigger<OnAdd, Session>,
    names: Query<&Name>,
    mut ui_state: ResMut<GlobalUi>,
) {
    let entity = trigger.entity();
    let name = names
        .get(entity)
        .expect("our session entity should have a name");
    ui_state.log.push(format!("{name} connected"));
}

fn on_disconnected(
    trigger: Trigger<Disconnected>,
    names: Query<&Name>,
    mut ui_state: ResMut<GlobalUi>,
) {
    let session = trigger.entity();
    let name = names
        .get(session)
        .expect("our session entity should have a name");
    ui_state.log.push(match &trigger.reason {
        DisconnectReason::User(reason) => {
            format!("{name} disconnected by user: {reason}")
        }
        DisconnectReason::Peer(reason) => {
            format!("{name} disconnected by peer: {reason}")
        }
        DisconnectReason::Error(err) => {
            format!("{name} disconnected due to error: {err:?}")
        }
    });
}

fn global_ui(
    mut commands: Commands,
    mut egui: EguiContexts,
    global_ui: Res<GlobalUi>,
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
        match sessions.get_single() {
            Ok((session, name, connected)) => {
                if connected.is_some() {
                    ui.label(format!("{name} connected"));
                } else {
                    ui.label(format!("{name} connecting"));
                }

                if ui.button("Disconnect").clicked() {
                    commands.trigger_targets(Disconnect::new("disconnected by user"), session);
                }
            }
            Err(QuerySingleError::NoEntities(_)) => {
                ui.label("No sessions active");
            }
            Err(QuerySingleError::MultipleEntities(_)) => {
                ui.label("Multiple sessions active");
            }
        }

        ui.separator();

        for msg in &global_ui.log {
            ui.label(msg);
        }
    });
}

fn web_transport_ui(
    mut commands: Commands,
    mut egui: EguiContexts,
    mut global_ui: ResMut<GlobalUi>,
    mut ui_state: ResMut<WebTransportUi>,
    sessions: Query<(), With<Session>>,
) {
    const DEFAULT_TARGET: &str = "https://remote-dev:25565";

    egui::Window::new("WebTransport").show(egui.ctx_mut(), |ui| {
        if sessions.iter().next().is_some() {
            ui.disable();
        }

        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

        let mut connect = false;
        ui.horizontal(|ui| {
            let connect_resp = ui.add(
                egui::TextEdit::singleline(&mut ui_state.target)
                    .hint_text(format!("{DEFAULT_TARGET} | [enter] to connect")),
            );
            connect |= connect_resp.lost_focus() && enter_pressed;
            connect |= ui.button("Connect").clicked();
        });

        let cert_hash_resp = ui.add(
            egui::TextEdit::singleline(&mut ui_state.cert_hash)
                .hint_text("(optional) certificate hash"),
        );
        connect |= cert_hash_resp.lost_focus() && enter_pressed;

        if connect {
            let mut target = ui_state.target.clone();
            if target.is_empty() {
                DEFAULT_TARGET.clone_into(&mut target);
            }

            let cert_hash = ui_state.cert_hash.clone();
            let config = web_transport_config(cert_hash);

            global_ui.session_id += 1;
            let name = format!("{}. {target}", global_ui.session_id);
            commands
                .spawn((Name::new(name), AeronetRepliconClient))
                .queue(WebTransportClient::connect(config, target));
        }
    });
}

fn web_socket_ui(
    mut commands: Commands,
    mut egui: EguiContexts,
    mut global_ui: ResMut<GlobalUi>,
    mut ui_state: ResMut<WebSocketUi>,
    sessions: Query<(), With<Session>>,
) {
    const DEFAULT_TARGET: &str = "ws://remote-dev:25566";

    egui::Window::new("WebSocket").show(egui.ctx_mut(), |ui| {
        if sessions.iter().next().is_some() {
            ui.disable();
        }

        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

        let mut connect = false;
        ui.horizontal(|ui| {
            let connect_resp = ui.add(
                egui::TextEdit::singleline(&mut ui_state.target)
                    .hint_text(format!("{DEFAULT_TARGET} | [enter] to connect")),
            );
            connect |= connect_resp.lost_focus() && enter_pressed;
            connect |= ui.button("Connect").clicked();
        });

        if connect {
            let mut target = ui_state.target.clone();
            if target.is_empty() {
                DEFAULT_TARGET.clone_into(&mut target);
            }

            let config = web_socket_config();

            global_ui.session_id += 1;
            let name = format!("{}. {target}", global_ui.session_id);
            commands
                .spawn((Name::new(name), AeronetRepliconClient))
                .queue(WebSocketClient::connect(config, target));
        }
    });
}
