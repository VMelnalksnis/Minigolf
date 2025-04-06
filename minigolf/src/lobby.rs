use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GameClientPacket {
    Hello,
    Available(SocketAddr),
    Busy,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GameServerPacket {
    Hello,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum UserClientPacket<'a> {
    Hello,
    CreateLobby,
    JoinLobby(&'a str),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum UserServerPacket<'a> {
    Hello,
    LobbyCreated(&'a str),
    JoinedLobby(&'a str),
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

impl Into<String> for UserClientPacket<'_> {
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl Into<String> for UserServerPacket<'_> {
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

impl<'a> From<&'a [u8]> for UserClientPacket<'a> {
    fn from(value: &'a [u8]) -> Self {
        serde_json::from_slice::<UserClientPacket>(value).unwrap()
    }
}

impl<'a> From<&'a [u8]> for UserServerPacket<'a> {
    fn from(value: &'a [u8]) -> Self {
        serde_json::from_slice::<UserServerPacket>(value).unwrap()
    }
}
