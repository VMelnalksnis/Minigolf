use {
    crate::{network::connect_to_lobby_server, ui::ServerState},
    aeronet::io::Session,
    bevy::prelude::*,
    bevy_egui::{EguiContexts, egui},
};

// UI for selecting the lobby server
pub(crate) struct LobbyServerUiPlugin;

impl Plugin for LobbyServerUiPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<LobbyServerSession>();

        app.init_resource::<LobbyServerUi>();

        app.configure_sets(
            Update,
            LobbyServerUiSet.run_if(in_state(ServerState::LobbyServer)),
        );

        app.add_systems(
            OnEnter(ServerState::LobbyServer),
            connect_to_default_lobby_server,
        )
        .add_systems(Update, lobby_server_ui.in_set(LobbyServerUiSet));

        app.add_observer(on_connected_to_lobby_server);
    }
}

/// Systems for selecting and connecting to a lobby server
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct LobbyServerUiSet;

#[derive(Resource, Reflect, Debug, Default)]
struct LobbyServerUi {
    target: String,
}

#[derive(Component, Reflect, Debug)]
pub(crate) struct LobbyServerSession;

const DEFAULT_LOBBY_TARGET: &str = "ws://localhost:25567";

fn connect_to_default_lobby_server(commands: Commands) {
    let target = DEFAULT_LOBBY_TARGET;
    connect_to_lobby_server(target, commands);
}

fn lobby_server_ui(
    commands: Commands,
    mut context: EguiContexts,
    mut ui_state: ResMut<LobbyServerUi>,
) {
    egui::Window::new("Select lobby server").show(context.ctx_mut(), |ui| {
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
            let target = match ui_state.target.is_empty() {
                true => DEFAULT_LOBBY_TARGET,
                false => ui_state.target.as_str(),
            };

            connect_to_lobby_server(target, commands);
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
