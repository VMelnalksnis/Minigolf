pub mod game;
pub mod user;

use {
    bevy::prelude::*,
    serde::{Deserialize, Serialize},
    uuid::Uuid,
};

#[derive(Serialize, Deserialize, Reflect, PartialEq, Clone, Copy, Hash, Debug)]
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
