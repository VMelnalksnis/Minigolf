use {
    crate::config::ServerPlugin,
    bevy::{
        app::ScheduleRunnerPlugin,
        prelude::*,
        render::{RenderPlugin, settings::WgpuSettings},
        winit::WinitPlugin,
    },
    minigolf::TICK_RATE,
    std::time::Duration,
};

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        backends: None,
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .disable::<WinitPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / f64::from(TICK_RATE),
        )));
    }
}
