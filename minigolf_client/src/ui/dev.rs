use {
    aeronet::io::{Session, SessionEndpoint, connection::Disconnect},
    bevy::prelude::*,
    bevy_egui::{EguiContexts, egui},
    bevy_replicon::prelude::*,
};

pub(crate) struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, network_stats_ui);
    }
}

fn network_stats_ui(
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
