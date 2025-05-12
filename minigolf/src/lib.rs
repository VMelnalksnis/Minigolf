pub mod lobby;
mod replication;

use {
    crate::{
        lobby::PlayerId,
        replication::{get_child_of_serialization_rules, register_replicated},
    },
    bevy::prelude::*,
    bevy_replicon::prelude::*,
    rand::{distr::StandardUniform, prelude::*},
    serde::{Deserialize, Serialize},
    uuid::Uuid,
};

/// How many times per second we will replicate entity components.
pub const TICK_RATE: u16 = 128;

/// Sets up replication and basic game systems.
#[derive(Debug)]
pub struct MinigolfPlugin;

/// Whether the game is currently being simulated or not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
#[states(scoped_entities)]
pub enum GameState {
    /// Game is not being simulated.
    #[default]
    None,
    /// Game is being simulated.
    Playing,
}

impl Plugin for MinigolfPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();

        app.register_type::<CourseDetails>();

        app.replicate::<Name>();
        app.replicate::<Transform>();
        app.replicate_with(get_child_of_serialization_rules());

        register_replicated::<Player>(app);
        register_replicated::<PlayerScore>(app);
        register_replicated::<PowerUp>(app);
        register_replicated::<PlayerPowerUps>(app);

        register_replicated::<LevelMesh>(app);
        register_replicated::<PlayableArea>(app);

        app.add_server_event::<RequestAuthentication>(Channel::Ordered);
        app.add_client_event::<AuthenticatePlayer>(Channel::Ordered);
        app.add_client_event::<PlayerInput>(Channel::Ordered);
    }
}

pub type CourseId = String;

#[derive(Reflect, Serialize, Deserialize, Clone, Debug)]
pub struct CourseDetails {
    pub id: CourseId,
    pub name: String,
}

/// Marker component for a player in the game.
#[derive(Component, Reflect, Serialize, Deserialize, Debug, Copy, Clone)]
#[require(StateScoped::<GameState>(GameState::Playing))]
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

#[derive(Component, Reflect, Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct PlayerCredentials {
    pub secret: String,
}

impl Default for PlayerCredentials {
    fn default() -> Self {
        PlayerCredentials {
            secret: Uuid::new_v4().into(),
        }
    }
}

/// Marker component for entities that the player can interact with.
#[derive(Component, Reflect, Serialize, Deserialize, Copy, Clone, Debug)]
pub struct PlayableArea;

#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug)]
#[require(StateScoped::<GameState>(GameState::Playing))]
pub struct LevelMesh {
    pub asset: String,
}

impl LevelMesh {
    pub fn from_path(path: &str) -> Self {
        LevelMesh { asset: path.into() }
    }
}

/// Player's inputs that they send to control their box.
#[derive(Event, Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum PlayerInput {
    /// Move in the specified direction with the specified force.
    Move(Vec2),

    /// Teleport to the specified position using the [PowerUpType::Teleport] power up.
    Teleport(Vec3),
    /// Apply an attractive force between the hole and the players ball using the [PowerUpType::HoleMagnet] power up.
    HoleMagnet,
    /// Apply hit force at a 45 degree angle for the next hit using the [PowerUpType::ChipShot] power up.
    ChipShot,

    /// Steal a power up from the specified player using the [PowerUpType::StealPowerUp] power up.
    StealPowerUp(PlayerId),

    StickyBall,
    TinyBall,
    HugeBall,
    ZanyBall,
    ReversiBall,

    Bumper(Vec3),
    BlackHoleBumper(Vec3),
    Tornado(Vec3),
    Wind(Vec2),
    StickyWalls,
    /// Make the floor of the current hole slippery using the [PowerUpType::IceRink] power up.
    IceRink,
}

impl PlayerInput {
    /// Whether the input is valid only when the player can move.
    pub fn is_movement(&self) -> bool {
        use PlayerInput::*;

        match self {
            Move(_) => true,
            _ => false,
        }
    }

    /// Gets the corresponding [PowerUpType].
    pub fn get_power_up_type(&self) -> Option<PowerUpType> {
        use PlayerInput::*;

        match self {
            Move(_) => None,
            Teleport(_) => Some(PowerUpType::Teleport),
            HoleMagnet => Some(PowerUpType::HoleMagnet),
            ChipShot => Some(PowerUpType::ChipShot),
            StealPowerUp(_) => Some(PowerUpType::StealPowerUp),
            StickyBall => Some(PowerUpType::StickyBall),
            TinyBall => Some(PowerUpType::TinyBall),
            HugeBall => Some(PowerUpType::HugeBall),
            ZanyBall => Some(PowerUpType::ZanyBall),
            ReversiBall => Some(PowerUpType::ReversiBall),
            Bumper(_) => Some(PowerUpType::Bumper),
            BlackHoleBumper(_) => Some(PowerUpType::BlackHoleBumper),
            Tornado(_) => Some(PowerUpType::Tornado),
            Wind(_) => Some(PowerUpType::Wind),
            StickyWalls => Some(PowerUpType::StickyWalls),
            IceRink => Some(PowerUpType::IceRink),
        }
    }
}

#[derive(Debug, Clone, Event, Serialize, Deserialize, Reflect)]
pub struct AuthenticatePlayer {
    pub id: PlayerId,
    pub credentials: PlayerCredentials,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize, Reflect)]
pub struct RequestAuthentication;

#[derive(Component, Reflect, Serialize, Deserialize, Default, Debug)]
pub struct PlayerScore {
    pub score: u32,
}

const PLAYER_POWER_UP_LIMIT: usize = 3;

#[derive(Component, Reflect, Serialize, Deserialize, Debug)]
pub struct PowerUp {
    pub power_up: PowerUpType,
}

impl From<PowerUpType> for PowerUp {
    fn from(value: PowerUpType) -> Self {
        PowerUp { power_up: value }
    }
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug)]
pub struct PlayerPowerUps {
    power_ups: Vec<PowerUpType>,
}

impl PlayerPowerUps {
    pub fn get_power_ups(&self) -> &[PowerUpType] {
        self.power_ups.as_slice()
    }

    pub fn add_power_up(&mut self, power_up: PowerUpType) -> Result<(), ()> {
        if self.power_ups.len() >= PLAYER_POWER_UP_LIMIT {
            Err(())
        } else {
            self.power_ups.push(power_up);
            Ok(())
        }
    }

    pub fn use_power_up(&mut self, power_up: PowerUpType) -> Option<PowerUpType> {
        if let Some(pos) = self.power_ups.iter().position(|x| *x == power_up) {
            Some(self.power_ups.remove(pos))
        } else {
            None
        }
    }
}

impl Default for PlayerPowerUps {
    fn default() -> Self {
        PlayerPowerUps {
            power_ups: IMPLEMENTED_POWER_UPS.to_vec(),
        }
    }
}

const IMPLEMENTED_POWER_UPS: [PowerUpType; 9] = [
    PowerUpType::Teleport,
    PowerUpType::HoleMagnet,
    PowerUpType::ChipShot,
    PowerUpType::StickyBall,
    PowerUpType::Bumper,
    PowerUpType::BlackHoleBumper,
    PowerUpType::Wind,
    PowerUpType::StickyWalls,
    PowerUpType::IceRink,
];

#[derive(Reflect, Serialize, Deserialize, PartialEq, Eq, Copy, Clone, Debug)]
pub enum PowerUpType {
    // Targeting self
    Teleport,
    HoleMagnet,
    GhostBall, // todo
    ChipShot,
    BallRepellent, // todo

    // Targeting specific player
    StealPowerUp, // todo

    // Targeting other players
    StickyBall,
    TinyBall,    // todo
    HugeBall,    // todo
    ZanyBall,    // todo
    ReversiBall, // todo

    // Targeting the environment
    Bumper,
    BlackHoleBumper,
    Tornado, // todo
    Wind,
    StickyWalls,
    IceRink,
}

impl Distribution<PowerUpType> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> PowerUpType {
        let index = rng.random_range(0..IMPLEMENTED_POWER_UPS.len());
        IMPLEMENTED_POWER_UPS[index]
    }
}
