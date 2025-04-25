use {
    crate::{
        PlayerCredentials,
        lobby::{LobbyId, PlayerId},
    },
    bevy::prelude::*,
    serde::{Deserialize, Serialize},
    uuid::Uuid,
};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ClientPacket {
    Hello,
    CreateLobby,
    ListLobbies,
    JoinLobby(LobbyId),
    LeaveLobby,
    StartGame,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ServerPacket {
    Hello(PlayerId, PlayerCredentials),
    LobbyCreated(LobbyId),
    AvailableLobbies(Vec<LobbyId>),
    LobbyJoined(LobbyId, Vec<PlayerId>),
    PlayerJoined(PlayerInLobby),
    PlayerLeft(PlayerInLobby),
    GameStarted(String),
}

#[derive(Serialize, Deserialize, Reflect, PartialEq, Copy, Clone, Debug)]
pub struct PlayerInLobby {
    pub lobby_id: LobbyId,
    pub player_id: PlayerId,
}

impl PlayerInLobby {
    pub fn new(lobby_id: LobbyId, player_id: PlayerId) -> Self {
        PlayerInLobby {
            lobby_id,
            player_id,
        }
    }
}

#[derive(Component, Reflect, Copy, Clone, Debug)]
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

impl Into<String> for ClientPacket {
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl Into<String> for ServerPacket {
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl<'a> From<&'a [u8]> for ClientPacket {
    fn from(value: &'a [u8]) -> Self {
        serde_json::from_slice::<ClientPacket>(value).unwrap()
    }
}

impl<'a> From<&'a [u8]> for ServerPacket {
    fn from(value: &'a [u8]) -> Self {
        serde_json::from_slice::<ServerPacket>(value).unwrap()
    }
}
