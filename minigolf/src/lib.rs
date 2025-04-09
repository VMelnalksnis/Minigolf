pub mod lobby;

use {
    crate::lobby::PlayerId,
    bevy::prelude::*,
    bevy_replicon::prelude::*,
    serde::{Deserialize, Serialize},
};

/// How many times per second we will replicate entity components.
pub const TICK_RATE: u16 = 128;

/// Sets up replication and basic game systems.
#[derive(Debug)]
pub struct MinigolfPlugin;

/// Whether the game is currently being simulated or not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
pub enum GameState {
    /// Game is not being simulated.
    #[default]
    None,
    /// Game is being simulated.
    Playing,
}

impl Plugin for MinigolfPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Player>()
            .register_type::<LevelMesh>()
            .register_type::<PlayerInput>()
            .init_state::<GameState>()
            .enable_state_scoped_entities::<GameState>()
            .replicate::<Player>()
            .replicate::<Transform>()
            .replicate::<LevelMesh>()
            .replicate::<Name>()
            .add_client_event::<PlayerInput>(ChannelKind::Unreliable);
    }
}

/// Marker component for a player in the game.
#[derive(Debug, Clone, Component, Serialize, Deserialize, Reflect)]
#[require(StateScoped<GameState>(|| StateScoped(GameState::Playing)))]
pub struct Player {
    pub id: PlayerId,
    pub can_move: bool,
}

impl Player {
    pub fn new() -> Self {
        Player {
            id: PlayerId::new(),
            can_move: false,
        }
    }
}

impl From<PlayerId> for Player {
    fn from(id: PlayerId) -> Self {
        Player {
            id,
            can_move: false,
        }
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, Reflect)]
#[require(StateScoped<GameState>(|| StateScoped(GameState::Playing)))]
pub struct LevelMesh {
    pub asset: String,
}

/// Player's inputs that they send to control their box.
#[derive(Debug, Clone, Default, Event, Serialize, Deserialize, Reflect)]
pub struct PlayerInput {
    /// Lateral movement vector.
    ///
    /// The client has full control over this field, and may send an
    /// unnormalized vector! Authorities must ensure that they normalize or
    /// zero this vector before using it for movement updates.
    pub movement: Vec2,
}
