use {
    crate::config::ServerPlugin,
    bevy::{
        input::{common_conditions::input_toggle_active, prelude::*},
        prelude::*,
    },
    bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin},
    iyes_perf_ui::prelude::*,
};

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins)
            .add_plugins(EguiPlugin)
            .add_plugins(
                WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
            )
            .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
            .add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin)
            .add_plugins(bevy::diagnostic::SystemInformationDiagnosticsPlugin)
            .add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin)
            .add_plugins(PerfUiPlugin)
            .add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        Transform::from_xyz(8.0, 0.0, 6.0),
    ));

    commands.spawn(PerfUiDefaultEntries::default());
}
