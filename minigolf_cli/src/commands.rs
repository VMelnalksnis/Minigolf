//! Commands, one per line. Empty lines and lines starting with `#` are ignored.
//!
//! Lobby, waits until connected to the lobby server:
//! - `create`, `list`, `join <lobby id>`, `leave`
//! - `start`, also waits until in a lobby
//!
//! Game, waits until the local ball has been able to move once, i.e. the hole is being played:
//! - `shoot <x> <z>`, also waits until the ball can move
//! - `teleport <x> <y> <z>`, `bumper <x> <y> <z>`, `black-hole <x> <y> <z>`, `wind <x> <z>`
//! - `magnet`, `chip`, `sticky-ball`, `sticky-walls`, `ice`
//!
//! Other:
//! - `wait <seconds>`
//! - `wait-ready`, waits until game commands can be used
//! - `wait-rest`, waits until the local ball has stopped moving after it started moving
//! - `dump`, prints all replicated entities
//! - `echo <text>`
//! - `quit`
//!
//! The client exits once all commands are done and the input is closed.

use {
    crate::{
        Args,
        network::{Authentication, CurrentLobby, LobbySession, send_to_lobby},
        output,
        report::{BallMotion, dump},
    },
    aeronet::io::Session,
    bevy::{ecs::system::SystemParam, prelude::*},
    minigolf::{
        PlayableArea, Player, PlayerInput,
        lobby::{LobbyId, user::ClientPacket},
    },
    std::{
        collections::VecDeque,
        io::BufRead,
        sync::{
            Mutex,
            mpsc::{self, Receiver, TryRecvError},
        },
    },
};

/// Runs commands from stdin or a script.
pub(crate) struct CommandsPlugin;

impl Plugin for CommandsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, open_input);
        app.add_systems(Update, run_commands);
    }
}

/// Time (s) after which a command that is not ready or a wait is given up on.
const TIMEOUT: f32 = 60.0;
/// Time (s) after which `wait-rest` is done if the ball did not start moving.
const REST_START_TIMEOUT: f32 = 1.0;

#[derive(Resource, Default)]
struct CommandQueue {
    lines: VecDeque<String>,
    /// Lines read from stdin, `None` when reading from a script.
    input: Option<Mutex<Receiver<String>>>,
    input_closed: bool,

    /// Since when the first command in the queue has not been ready.
    blocked_since: Option<f32>,
    wait: Option<Wait>,
    /// Whether the local ball has been able to move, i.e. game commands can be used.
    game_ready: bool,
    exiting: bool,
}

struct Wait {
    kind: WaitKind,
    started: f32,
}

enum WaitKind {
    Until(f32),
    Ready,
    Rest { seen_moving: bool },
}

fn open_input(args: Res<Args>, mut commands: Commands) {
    let mut queue = CommandQueue::default();

    match &args.script {
        Some(path) => {
            let script = std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("could not read script {path:?}: {error}"));
            queue.lines.extend(script.lines().map(String::from));
            queue.input_closed = true;
        }

        None => {
            let (sender, receiver) = mpsc::channel();
            std::thread::spawn(move || {
                for line in std::io::stdin().lock().lines() {
                    let Ok(line) = line else {
                        break;
                    };

                    if sender.send(line).is_err() {
                        break;
                    }
                }
            });
            queue.input = Some(Mutex::new(receiver));
        }
    }

    commands.insert_resource(queue);
}

enum Command {
    Lobby(ClientPacket),
    Input(PlayerInput),
    Wait(WaitKind),
    Dump,
    Echo(String),
    Quit,
}

fn parse(line: &str) -> Result<Command, String> {
    let mut words = line.split_whitespace();
    let name = words.next().unwrap_or_default();
    let args = words.collect::<Vec<_>>();

    let numbers = |count: usize| -> Result<Vec<f32>, String> {
        if args.len() != count {
            return Err(format!("`{name}` takes {count} numbers"));
        }

        args.iter()
            .map(|arg| arg.parse::<f32>().map_err(|_| format!("`{arg}` is not a number")))
            .collect()
    };
    let vec2 = || numbers(2).map(|n| Vec2::new(n[0], n[1]));
    let vec3 = || numbers(3).map(|n| Vec3::new(n[0], n[1], n[2]));

    let command = match name {
        "create" => Command::Lobby(ClientPacket::CreateLobby),
        "list" => Command::Lobby(ClientPacket::ListLobbies),
        "join" => {
            let id = args
                .first()
                .and_then(|id| id.parse::<LobbyId>().ok())
                .ok_or("`join` takes a lobby id")?;
            Command::Lobby(ClientPacket::JoinLobby(id))
        }
        "leave" => Command::Lobby(ClientPacket::LeaveLobby),
        "start" => Command::Lobby(ClientPacket::StartGame),

        "shoot" => Command::Input(PlayerInput::Move(vec2()?)),
        "teleport" => Command::Input(PlayerInput::Teleport(vec3()?)),
        "bumper" => Command::Input(PlayerInput::Bumper(vec3()?)),
        "black-hole" => Command::Input(PlayerInput::BlackHoleBumper(vec3()?)),
        "wind" => Command::Input(PlayerInput::Wind(vec2()?)),
        "magnet" => Command::Input(PlayerInput::HoleMagnet),
        "chip" => Command::Input(PlayerInput::ChipShot),
        "sticky-ball" => Command::Input(PlayerInput::StickyBall),
        "sticky-walls" => Command::Input(PlayerInput::StickyWalls),
        "ice" => Command::Input(PlayerInput::IceRink),

        "wait" => Command::Wait(WaitKind::Until(crate::elapsed() + numbers(1)?[0])),
        "wait-ready" => Command::Wait(WaitKind::Ready),
        "wait-rest" => Command::Wait(WaitKind::Rest { seen_moving: false }),
        "dump" => Command::Dump,
        "echo" => Command::Echo(args.join(" ")),
        "quit" => Command::Quit,

        _ => return Err(format!("unknown command `{name}`")),
    };

    Ok(command)
}

#[derive(SystemParam)]
struct Context<'w, 's> {
    lobby: Query<'w, 's, &'static mut Session, With<LobbySession>>,
    authentication: Option<Res<'w, Authentication>>,
    current_lobby: ResMut<'w, CurrentLobby>,
    players: Query<'w, 's, (Entity, &'static Player)>,
    holes: Query<'w, 's, (), With<PlayableArea>>,
    motion: Res<'w, BallMotion>,
    inputs: MessageWriter<'w, PlayerInput>,
    exit: MessageWriter<'w, AppExit>,
    commands: Commands<'w, 's>,
}

impl Context<'_, '_> {
    fn local_player(&self) -> Option<(Entity, &Player)> {
        let authentication = self.authentication.as_ref()?;

        self.players
            .iter()
            .find(|(_, player)| player.id == authentication.id)
    }
}

enum Outcome {
    Done,
    NotReady,
    Wait(WaitKind),
}

fn execute(command: Command, queue: &CommandQueue, context: &mut Context) -> Outcome {
    match command {
        Command::Lobby(packet) => {
            // The lobby server sends the player id and credentials after connecting.
            if context.authentication.is_none() {
                return Outcome::NotReady;
            }

            // The lobby server can not handle starting a game in the same frame as creating or
            // joining a lobby, so wait until it has replied.
            if packet == ClientPacket::StartGame && context.current_lobby.0.is_none() {
                return Outcome::NotReady;
            }

            if packet == ClientPacket::LeaveLobby {
                context.current_lobby.0 = None;
            }

            if !send_to_lobby(&mut context.lobby, packet) {
                return Outcome::NotReady;
            }
        }

        Command::Input(input) => {
            let Some((_, player)) = context.local_player() else {
                return Outcome::NotReady;
            };

            if !queue.game_ready || (input.is_movement() && !player.can_move) {
                return Outcome::NotReady;
            }

            context.inputs.write(input);
        }

        Command::Wait(kind) => return Outcome::Wait(kind),

        Command::Dump => context.commands.run_system_cached(dump),
        Command::Echo(text) => output!("echo {text}"),
        Command::Quit => {
            context.exit.write(AppExit::Success);
        }
    }

    Outcome::Done
}

/// Whether the wait is over.
fn wait_done(wait: &mut Wait, queue_game_ready: bool, context: &Context) -> bool {
    let now = crate::elapsed();

    if now - wait.started > TIMEOUT {
        output!("error wait timed out");
        return true;
    }

    match &mut wait.kind {
        WaitKind::Until(until) => now >= *until,
        WaitKind::Ready => queue_game_ready,
        WaitKind::Rest { seen_moving } => {
            let Some((ball, _)) = context.local_player() else {
                return false;
            };

            if context.motion.is_moving(ball) {
                *seen_moving = true;
                return false;
            }

            *seen_moving || now - wait.started > REST_START_TIMEOUT
        }
    }
}

fn run_commands(mut queue: ResMut<CommandQueue>, mut context: Context) {
    let queue = &mut *queue;

    if let Some(input) = &queue.input {
        let input = input.lock().unwrap();
        loop {
            match input.try_recv() {
                Ok(line) => queue.lines.push_back(line),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    queue.input_closed = true;
                    break;
                }
            }
        }
    }

    if context
        .local_player()
        .is_some_and(|(_, player)| player.can_move)
        && !context.holes.is_empty()
    {
        queue.game_ready = true;
    }

    loop {
        if let Some(wait) = &mut queue.wait {
            if !wait_done(wait, queue.game_ready, &context) {
                return;
            }

            queue.wait = None;
        }

        let Some(line) = queue.lines.front().cloned() else {
            if queue.input_closed && !queue.exiting {
                queue.exiting = true;
                output!("done");
                context.exit.write(AppExit::Success);
            }
            return;
        };

        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            queue.lines.pop_front();
            continue;
        }

        let command = match parse(line) {
            Ok(command) => command,
            Err(error) => {
                output!("error {error}: {line}");
                queue.lines.pop_front();
                continue;
            }
        };

        match execute(command, queue, &mut context) {
            Outcome::NotReady => {
                let now = crate::elapsed();
                let blocked_since = *queue.blocked_since.get_or_insert(now);

                if now - blocked_since > TIMEOUT {
                    output!("error command not ready, skipping: {line}");
                    queue.blocked_since = None;
                    queue.lines.pop_front();
                    continue;
                }

                return;
            }

            Outcome::Done => {
                output!("> {line}");
            }

            Outcome::Wait(kind) => {
                output!("> {line}");
                queue.wait = Some(Wait {
                    kind,
                    started: crate::elapsed(),
                });
            }
        }

        queue.blocked_since = None;
        queue.lines.pop_front();
    }
}
