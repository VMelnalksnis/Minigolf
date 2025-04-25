use {
    crate::ui::{ServerState, lobby_server::LobbyServerSession},
    aeronet::io::{Session, bytes::Bytes},
    bevy::prelude::*,
    bevy_egui::{EguiContexts, egui},
    minigolf::lobby::{LobbyId, user::ClientPacket},
};

/// UI for creating/selecting a lobby
pub(crate) struct LobbySelectUiPlugin;

impl Plugin for LobbySelectUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LobbiesUi>();

        app.configure_sets(Update, LobbiesUiSet.run_if(in_state(ServerState::Lobbies)));

        app.add_systems(Update, lobbies_ui.in_set(LobbiesUiSet));
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct LobbiesUiSet;

#[derive(Resource, Reflect, Debug, Default)]
struct LobbiesUi {
    lobby_id: String,
}

fn lobbies_ui(
    mut context: EguiContexts,
    mut lobbies_ui: ResMut<LobbiesUi>,
    mut lobby_session: Query<&mut Session, With<LobbyServerSession>>,
) {
    egui::Window::new("Select lobby").show(context.ctx_mut(), |ui| {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut lobbies_ui.lobby_id);

            if ui.button("Join lobby").clicked() {
                let Ok(id) = lobbies_ui.lobby_id.parse::<LobbyId>() else {
                    lobbies_ui.lobby_id = String::new();
                    return;
                };

                info!("Joining lobby {}", lobbies_ui.lobby_id);

                let mut session = lobby_session.single_mut().unwrap();
                let request: String = ClientPacket::JoinLobby(id).into();
                session.send.push(Bytes::from(request));
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Create lobby").clicked() {
                info!("Creating lobby");

                let mut session = lobby_session.single_mut().unwrap();
                let request: String = ClientPacket::CreateLobby.into();
                session.send.push(Bytes::from(request));
            }
        })
    });
}
