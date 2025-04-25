mod game;
mod user;

use {
    crate::{game::GameServerPlugin, user::UserPlugin},
    aeronet_websocket::server::WebSocketServerPlugin,
    bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*},
    core::time::Duration,
    minigolf::{
        Player,
        lobby::user::{LobbyMember, PlayerInLobby},
    },
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
        .add_observer(on_lobby_member_removed)
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

impl Lobby {
    fn new(owner: Entity) -> Self {
        Lobby { owner }
    }
}

#[derive(Event, Reflect, Deref, DerefMut, Debug)]
struct PlayerJoinedLobby(PlayerInLobby);

#[derive(Event, Reflect, Deref, DerefMut, Debug)]
struct PlayerDisconnected(PlayerInLobby);

fn on_lobby_member_removed(
    trigger: Trigger<OnRemove, LobbyMember>,
    members: Query<(Entity, &LobbyMember), Without<Lobby>>,
    lobby: Query<(Entity, &LobbyMember), With<Lobby>>,
    players: Query<&Player>,
    mut commands: Commands,
) {
    let entity = trigger.target();
    let Ok((_, lobby_member)) = members.get(entity) else {
        return;
    };

    let id = lobby_member.lobby_id;
    info!("{:?} left lobby {:?}", entity, id);

    if let Ok(player) = players.get(entity) {
        commands.trigger(PlayerDisconnected(PlayerInLobby::new(id, player.id)));
    }

    let Some(lobby_entity) = lobby
        .iter()
        .filter(|(_, l)| l.lobby_id == id)
        .map(|(entity, _)| entity)
        .next()
    else {
        info!("No lobby found with id {:?}", id);
        return;
    };

    let member_count = members
        .iter()
        .filter(|(e, m)| *e != entity && m.lobby_id == id)
        .count();

    info!("{:?} members remaining in lobby {:?}", member_count, id);

    if member_count == 0 {
        info!("Deleting lobby {:?}", id);
        commands.entity(lobby_entity).despawn();
    }
}
