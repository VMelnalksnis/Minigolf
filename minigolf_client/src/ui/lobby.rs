use {
    crate::ui::{ServerState, lobby_server::LobbyServerSession},
    aeronet::io::{Session, bytes::Bytes},
    bevy::prelude::*,
    bevy_egui::{EguiContexts, egui},
    minigolf::lobby::{PlayerId, UserClientPacket},
};

// UI for managing the current lobby
pub(crate) struct LobbyUiPlugin;

impl Plugin for LobbyUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LobbyUi>();

        app.configure_sets(Update, LobbyUiSet.run_if(in_state(ServerState::Lobby)))
            .add_systems(Update, lobby_ui.in_set(LobbyUiSet));
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct LobbyUiSet;

#[derive(Resource, Reflect, Debug, Default)]
struct LobbyUi {
    lobby_id: String,
    players: Vec<PlayerId>,
}

fn lobby_ui(
    mut context: EguiContexts,
    lobby_ui: ResMut<LobbyUi>,
    mut lobby_session: Query<&mut Session, With<LobbyServerSession>>,
    mut state: ResMut<NextState<ServerState>>,
) {
    let title = format!("Lobby {}", lobby_ui.lobby_id);

    egui::Window::new(title).show(context.ctx_mut(), |ui| {
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
