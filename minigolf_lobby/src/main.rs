mod game;
mod user;

use {
    crate::{game::GameServerPlugin, user::UserPlugin},
    aeronet_websocket::server::WebSocketServerPlugin,
    bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*},
    core::time::Duration,
    std::net::{IpAddr, Ipv6Addr, SocketAddr},
};

const TICK_RATE: f64 = 32.0;

fn main() -> AppExit {
    App::new()
        .init_resource::<Args>()
        .add_plugins(LogPlugin::default())
        .add_plugins(
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / TICK_RATE,
            ))),
        )
        .add_plugins(WebSocketServerPlugin)
        .add_plugins((GameServerPlugin, UserPlugin))
        .insert_resource(Time::<Fixed>::from_hz(TICK_RATE))
        .run()
}

const USER_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 25567);
const GAME_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 25568);

/// minigolf lobby server
#[derive(Debug, Resource, clap::Parser)]
struct Args {
    /// Address to listen on for users
    #[arg(long, default_value_t = USER_ADDRESS)]
    user_address: SocketAddr,

    /// Address to listen on for game servers
    #[arg(long, default_value_t = GAME_ADDRESS)]
    game_address: SocketAddr,
}

impl FromWorld for Args {
    fn from_world(_: &mut World) -> Self {
        <Self as clap::Parser>::parse()
    }
}

#[derive(Debug, Component, Reflect)]
struct Lobby {
    owner: Entity,
}
