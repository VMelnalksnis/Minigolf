use bevy::prelude::Reflect;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Reflect, Copy)]
pub struct UniqueId {
    id: Uuid,
}

impl UniqueId {
    pub fn new() -> Self {
        UniqueId { id: Uuid::new_v4() }
    }
}

pub type PlayerId = UniqueId;
pub type LobbyId = u64;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Reflect)]
pub struct PlayerInLobby {
    pub lobby_id: LobbyId,
    pub player_id: PlayerId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GameClientPacket {
    Hello,
    Available(SocketAddr),
    Busy,
    GameCreated(LobbyId),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GameServerPacket {
    Hello,
    CreateGame(LobbyId),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum UserClientPacket {
    Hello,
    CreateLobby,
    ListLobbies,
    JoinLobby(LobbyId),
    StartGame,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum UserServerPacket {
    Hello,
    LobbyCreated(LobbyId),
    AvailableLobbies(Vec<LobbyId>),
    LobbyJoined(LobbyId),
    PlayerJoined(PlayerInLobby),
    PlayerLeft(PlayerInLobby),
    GameStarted(LobbyId),
}

// helpers for simplifying sending/receiving code

impl Into<String> for GameClientPacket {
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl Into<String> for GameServerPacket {
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl Into<String> for UserClientPacket {
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl Into<String> for UserServerPacket {
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl<'a> From<&'a [u8]> for GameClientPacket {
    fn from(value: &'a [u8]) -> Self {
        serde_json::from_slice::<GameClientPacket>(value).unwrap()
    }
}

impl<'a> From<&'a [u8]> for GameServerPacket {
    fn from(value: &'a [u8]) -> Self {
        serde_json::from_slice::<GameServerPacket>(value).unwrap()
    }
}

impl<'a> From<&'a [u8]> for UserClientPacket {
    fn from(value: &'a [u8]) -> Self {
        serde_json::from_slice::<UserClientPacket>(value).unwrap()
    }
}

impl<'a> From<&'a [u8]> for UserServerPacket {
    fn from(value: &'a [u8]) -> Self {
        serde_json::from_slice::<UserServerPacket>(value).unwrap()
    }
}
