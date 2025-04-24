use {
    crate::PlayerCredentials,
    bevy::prelude::*,
    serde::{Deserialize, Serialize},
    uuid::Uuid,
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Reflect, Copy, Hash)]
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

#[derive(Debug, Component, Reflect, Copy, Clone)]
pub struct LobbyMember {
    pub lobby_id: LobbyId,
}

impl LobbyMember {
    pub fn new() -> Self {
        LobbyMember {
            lobby_id: Uuid::new_v4().as_u64_pair().0,
        }
    }
}

impl From<LobbyId> for LobbyMember {
    fn from(value: LobbyId) -> Self {
        LobbyMember { lobby_id: value }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GameClientPacket {
    Hello,
    Available(String),
    Busy,
    GameCreated(LobbyId),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GameServerPacket {
    Hello,
    CreateGame(LobbyId, Vec<(PlayerId, PlayerCredentials)>),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum UserClientPacket {
    Hello,
    CreateLobby,
    ListLobbies,
    JoinLobby(LobbyId),
    LeaveLobby,
    StartGame,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum UserServerPacket {
    Hello(PlayerId, PlayerCredentials),
    LobbyCreated(LobbyId),
    AvailableLobbies(Vec<LobbyId>),
    LobbyJoined(LobbyId),
    PlayerJoined(PlayerInLobby),
    PlayerLeft(PlayerInLobby),
    GameStarted(String),
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
