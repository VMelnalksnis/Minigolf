use {
    crate::ui::{ServerState, lobby_server::LobbyServerSession},
    aeronet::io::{Session, bytes::Bytes},
    bevy::prelude::*,
    bevy_egui::{EguiContexts, egui},
    minigolf::lobby::{PlayerId, user::ClientPacket},
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
pub(crate) struct LobbyUi {
    lobby_id: String,
    player_ids: Vec<PlayerId>,
}

impl LobbyUi {
    pub(crate) fn new_lobby(lobby_id: String) -> Self {
        LobbyUi {
            lobby_id,
            player_ids: vec![],
        }
    }

    pub(crate) fn new_existing_lobby(lobby_id: String, player_ids: Vec<PlayerId>) -> Self {
        LobbyUi {
            lobby_id,
            player_ids,
        }
    }

    pub(crate) fn add_player(&mut self, player: PlayerId) {
        info!("Player joined current lobby {:?}", player);
        self.player_ids.push(player);
    }

    pub(crate) fn remove_player(&mut self, player: PlayerId) {
        info!("Player left current lobby {:?}", player);
        self.player_ids.retain(|p| *p != player);
    }
}

fn lobby_ui(
    mut context: EguiContexts,
    lobby_ui: ResMut<LobbyUi>,
    mut lobby_session: Query<&mut Session, With<LobbyServerSession>>,
    mut state: ResMut<NextState<ServerState>>,
) {
    egui::Window::new("Lobby").show(context.ctx_mut(), |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("Lobby ID: {}", lobby_ui.lobby_id));
        });
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Start game").clicked() {
                info!("Starting game");

                let mut session = lobby_session.single_mut().unwrap();
                let request: String = ClientPacket::StartGame.into();
                session.send.push(Bytes::from(request));
            }

            if ui.button("Leave lobby").clicked() {
                info!("Leaving lobby");

                let mut session = lobby_session.single_mut().unwrap();
                let request: String = ClientPacket::LeaveLobby.into();
                session.send.push(Bytes::from(request));
                state.set(ServerState::Lobbies);
            }
        });
        ui.separator();

        ui.label("Players");
        for player in &lobby_ui.player_ids {
            ui.label(format!("{player:?}"));
        }
    });
}
