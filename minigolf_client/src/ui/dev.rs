use {
    aeronet::io::{connection::Disconnect, Session, SessionEndpoint},
    bevy::prelude::*,
    bevy_egui::{egui, EguiContexts},
    bevy_replicon::prelude::*,
};

pub(crate) struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, network_stats_ui);

        app.add_plugins(bevy_inspector_egui::quick::WorldInspectorPlugin::default());
    }
}

fn network_stats_ui(
    mut commands: Commands,
    mut egui: EguiContexts,
    sessions: Query<(Entity, &Name, Option<&Session>), With<SessionEndpoint>>,
    client_state: Res<State<ClientState>>,
    stats: Res<ClientStats>,
) {
    egui::Window::new("Session Log").show(egui.ctx_mut().unwrap(), |ui| {
        ui.label("Replicon reports:");
        ui.horizontal(|ui| {
            ui.label(match client_state.get() {
                ClientState::Disconnected => "Disconnected",
                ClientState::Connecting => "Connecting",
                ClientState::Connected => "Connected",
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
                    commands.trigger(Disconnect::new(session, "disconnected by user"));
                }
            });
        }
    });
}
