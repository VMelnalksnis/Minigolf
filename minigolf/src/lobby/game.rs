use {
    crate::{
        PlayerCredentials,
        lobby::{LobbyId, PlayerId},
    },
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ClientPacket {
    Hello,
    Available(String),
    Busy,
    GameCreated(LobbyId),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ServerPacket {
    Hello,
    CreateGame(LobbyId, Vec<(PlayerId, PlayerCredentials)>),
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
