#[cfg(feature = "dev")]
mod dev;
pub(crate) mod lobby;
mod lobby_select;
pub(crate) mod lobby_server;
mod power_ups;

use {
    crate::ui::{
        lobby::LobbyUiPlugin, lobby_select::LobbySelectUiPlugin, lobby_server::LobbyServerUiPlugin,
        power_ups::PowerUpUiPlugin,
    },
    bevy::prelude::*,
    bevy_egui::EguiPlugin,
};

/// Sets up minigolf client UI.
pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: false,
        });

        #[cfg(feature = "dev")]
        {
            app.add_plugins(dev::DebugUiPlugin);
        }

        app.add_plugins((
            LobbyServerUiPlugin,
            LobbySelectUiPlugin,
            LobbyUiPlugin,
            PowerUpUiPlugin,
        ));

        app.init_state::<ServerState>();
    }
}

#[derive(States, Reflect, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ServerState {
    #[default]
    LobbyServer,
    Lobbies,
    Lobby,
    GameServer,
}
