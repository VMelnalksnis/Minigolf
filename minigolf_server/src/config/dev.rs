use {
    crate::config::ServerPlugin, bevy::prelude::*, bevy_egui::EguiPlugin,
    bevy_inspector_egui::quick::WorldInspectorPlugin,
};

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins)
            .add_plugins(EguiPlugin {
                enable_multipass_for_primary_context: false,
            })
            .add_plugins(WorldInspectorPlugin::new())
            .add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        Transform::from_xyz(3.0, 0.0, 10.0),
    ));
}
