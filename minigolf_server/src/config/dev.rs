use {
    crate::config::ServerPlugin,
    bevy::{
        input::{common_conditions::input_toggle_active, prelude::*},
        prelude::*,
    },
    bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin},
};

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins)
            .add_plugins(EguiPlugin)
            .add_plugins(
                WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
            );
    }
}
