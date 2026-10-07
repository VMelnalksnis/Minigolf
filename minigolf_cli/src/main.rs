//! Command line client for testing the game without rendering.
//!
//! Prints what happens in the game to stdout, one event per line, and reads commands from stdin
//! (or a script file). Logs are written to stderr. See [commands] for the available commands.

mod commands;
mod network;
mod report;

use {
    crate::{commands::CommandsPlugin, network::CliNetworkPlugin, report::ReportPlugin},
    bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, state::app::StatesPlugin},
    minigolf::MinigolfPlugin,
    std::{
        path::PathBuf,
        sync::OnceLock,
        time::{Duration, Instant},
    },
};

/// minigolf command line client
#[derive(Resource, clap::Parser, Debug)]
struct Args {
    /// Address of the lobby server
    #[arg(long, default_value = "ws://localhost:25567")]
    lobby: String,

    /// Read commands from this file instead of stdin
    #[arg(long)]
    script: Option<PathBuf>,
}

fn main() -> AppExit {
    let args = <Args as clap::Parser>::parse();
    START.get_or_init(Instant::now);

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 128.0,
            ))),
            LogPlugin {
                // Keep stderr quiet, use `RUST_LOG` for more.
                level: bevy::log::Level::WARN,
                ..default()
            },
            StatesPlugin,
            TransformPlugin,
        ))
        .add_plugins((
            CliNetworkPlugin,
            MinigolfPlugin,
            ReportPlugin,
            CommandsPlugin,
        ))
        .insert_resource(args)
        .run()
}

static START: OnceLock<Instant> = OnceLock::new();

/// Seconds since the client started.
fn elapsed() -> f32 {
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// Prints a line of output, prefixed with the time since the client started.
macro_rules! output {
    ($($arg:tt)*) => {
        println!("[{:8.3}] {}", crate::elapsed(), format_args!($($arg)*))
    };
}

pub(crate) use output;
