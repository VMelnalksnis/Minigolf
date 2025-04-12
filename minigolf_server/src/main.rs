use bevy::prelude::States;

#[cfg(target_family = "wasm")]
fn main() {
    panic!("this example is not available on WASM");
}

#[cfg(not(target_family = "wasm"))]
mod network;

#[cfg(not(target_family = "wasm"))]
mod server;

mod config;
mod course;

#[cfg(not(target_family = "wasm"))]
fn main() {
    server::main();
}

#[derive(States, Default, Clone, Eq, PartialEq, Hash, Debug)]
enum ServerState {
    #[default]
    WaitingForLobby,
    WaitingForGame,
    WaitingForPlayers,
    Playing,
}
