mod network;
mod ui;

use {
    crate::{network::ClientNetworkPlugin, ui::ClientUiPlugin},
    aeronet::io::{Session, connection::Disconnected},
    bevy::{
        color::palettes::basic::RED,
        ecs::query::QuerySingleError,
        input::{
            common_conditions::{input_just_released, input_pressed},
            mouse::{MouseMotion, MouseWheel},
            touch::TouchPhase,
        },
        prelude::*,
        window::PrimaryWindow,
    },
    minigolf::{GameState, LevelMesh, MinigolfPlugin, Player, PlayerInput},
    web_sys::{HtmlCanvasElement, wasm_bindgen::JsCast},
};

fn main() -> AppExit {
    App::new()
        .register_type::<LocalPlayer>()
        .register_type::<AccumulatedInputs>()
        .add_plugins((
            DefaultPlugins,
            ClientUiPlugin,
            ClientNetworkPlugin,
            MinigolfPlugin,
        ))
        .add_systems(Startup, (set_window_title, setup_level))
        .init_resource::<TouchState>()
        .init_state::<InputState>()
        .add_systems(
            Update,
            (
                handle_inputs.in_set(InputSet),
                handle_touch.in_set(InputSet),
                launch_inputs
                    .in_set(InputSet)
                    .run_if(input_pressed(MouseButton::Left)),
                handle_mouse
                    .in_set(InputSet)
                    .run_if(input_just_released(MouseButton::Left)),
                cancel_mouse
                    .in_set(InputSet)
                    .run_if(input_just_released(MouseButton::Right)),
                camera_follow.run_if(in_state(GameState::Playing)),
                draw_gizmos.in_set(InputSet),
            ),
        )
        .add_systems(Update, (test, scroll_events, follow_player))
        .configure_sets(
            Update,
            InputSet.run_if(in_state(GameState::Playing).and(in_state(InputState::CanMove))),
        )
        .add_observer(on_connected)
        .add_observer(on_player_added)
        .add_observer(on_level_mesh_added)
        .add_observer(on_disconnected)
        .run()
}

fn set_window_title(mut primary_windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = primary_windows.get_single_mut() {
        window.title = "Minigolf".to_string();
    }
}

fn follow_player(
    player: Query<&Transform, With<LocalPlayer>>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<LocalPlayer>)>,
) {
    let Ok(mut camera) = camera.get_single_mut() else {
        return;
    };

    let position = match player.get_single() {
        Ok(position) => position.translation.x,
        _ => 0.0,
    };

    camera.translation.x = position - 2.5;
}

fn test(
    query: Query<&Player, (Changed<Player>, With<LocalPlayer>)>,
    mut input_state: ResMut<NextState<InputState>>,
) {
    let Ok(player) = query.get_single() else {
        return;
    };

    let next_state = match player.can_move {
        true => InputState::CanMove,
        false => InputState::CannotMove,
    };

    input_state.set(next_state);
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct InputSet;

#[derive(States, Reflect, Default, Debug, Clone, PartialEq, Eq, Hash)]
enum InputState {
    #[default]
    CanMove,
    CannotMove,
}

#[derive(Debug, Component, Reflect)]
struct LocalPlayer;

#[derive(Debug, Clone, Component, Deref, DerefMut, Reflect)]
struct AccumulatedInputs {
    input: Vec2,
}

fn setup_level(mut commands: Commands) {
    if cfg!(target_family = "wasm") {
        let canvas: HtmlCanvasElement = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .query_selector("canvas")
            .unwrap()
            .unwrap()
            .unchecked_into();
        let style = canvas.style();
        style.set_property("width", "100%").unwrap();
        style.set_property("height", "100%").unwrap();
    }

    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 1.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn on_player_added(
    trigger: Trigger<OnAdd, Player>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    players: Query<(), With<LocalPlayer>>,
    all_players: Query<(Entity, &Player)>,
    authentication: Res<ui::Authentication>,
) {
    let entity = trigger.entity();
    let player_mesh_handle: Handle<Mesh> = server.load("Player.glb#Mesh0/Primitive0");

    commands.entity(entity).insert((
        Mesh3d(player_mesh_handle.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("#ffd891").unwrap().into(),
            metallic: 0.5,
            perceptual_roughness: 0.5,
            ..default()
        })),
    ));

    if let Err(QuerySingleError::NoEntities(_)) = players.get_single() {
        let x = all_players
            .iter()
            .filter(|(e, p)| *e == entity && p.id == authentication.id)
            .map(|(e, _)| e)
            .collect::<Vec<_>>();

        if let &[_] = x.as_slice() {
            commands
                .entity(entity)
                .insert((LocalPlayer, AccumulatedInputs { input: Vec2::ZERO }));
        }
    }
}

fn on_level_mesh_added(
    trigger: Trigger<OnAdd, LevelMesh>,
    query: Query<&LevelMesh>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let entity = trigger.entity();
    let level_mesh = query.get(entity).unwrap();
    let mesh_handle: Handle<Mesh> = server.load(level_mesh.clone().asset);

    commands.entity(entity).insert((
        Mesh3d(mesh_handle.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            metallic: 0.5,
            perceptual_roughness: 0.5,
            ..default()
        })),
    ));
}

fn on_connected(_trigger: Trigger<OnAdd, Session>, mut game_state: ResMut<NextState<GameState>>) {
    game_state.set(GameState::Playing);
}

fn on_disconnected(_trigger: Trigger<Disconnected>, mut game_state: ResMut<NextState<GameState>>) {
    game_state.set(GameState::None);
}

fn handle_inputs(mut inputs: EventWriter<PlayerInput>, input: Res<ButtonInput<KeyCode>>) {
    let mut movement = Vec2::ZERO;
    if input.just_released(KeyCode::ArrowRight) {
        movement.x += 1.0;
    }
    if input.just_released(KeyCode::ArrowLeft) {
        movement.x -= 1.0;
    }
    if input.just_released(KeyCode::ArrowUp) {
        movement.y += 1.0;
    }
    if input.just_released(KeyCode::ArrowDown) {
        movement.y -= 1.0;
    }
    if movement == Vec2::ZERO {
        return;
    }

    // don't normalize here, since the server will normalize anyway
    inputs.send(PlayerInput { movement });
}

#[derive(Resource, Reflect, Debug, Default)]
struct TouchState {
    start: Option<Vec2>,
    last: Option<Vec2>,
}

fn handle_touch(
    mut touch_inputs: EventReader<TouchInput>,
    mut inputs: Query<&mut AccumulatedInputs, With<LocalPlayer>>,
    mut state: ResMut<TouchState>,
    mut writer: EventWriter<PlayerInput>,
) {
    for touch in touch_inputs.read() {
        let Ok(mut input) = inputs.get_single_mut() else {
            continue;
        };

        match touch.phase {
            TouchPhase::Started => {
                state.start = Some(touch.position);
                input.input = Vec2::ZERO;
            }
            TouchPhase::Moved => {
                let delta = match state.last {
                    None => Vec2::ZERO,
                    Some(last) => touch.position - last,
                };

                input.input.y -= delta.x / 100.0;
                input.input.x += delta.y / 100.0;

                input.input = input.input.clamp_length_max(1.0);

                state.last = Some(touch.position);
            }
            TouchPhase::Ended => {
                if input.input == Vec2::ZERO {
                    continue;
                }

                writer.send(PlayerInput {
                    movement: input.input,
                });

                input.input = Vec2::ZERO;
                state.start = None;
                state.last = None;
            }
            TouchPhase::Canceled => {
                state.start = None;
            }
        }
    }
}

fn launch_inputs(
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut inputs: Query<&mut AccumulatedInputs, With<LocalPlayer>>,
) {
    for ev in mouse_motion_events.read() {
        let Ok(mut input) = inputs.get_single_mut() else {
            continue;
        };

        input.input.y -= ev.delta.x / 100.0;
        input.input.x += ev.delta.y / 100.0;

        input.input = input.input.clamp_length_max(1.0);
    }
}

fn handle_mouse(
    mut writer: EventWriter<PlayerInput>,
    mut inputs: Query<&mut AccumulatedInputs, With<LocalPlayer>>,
) {
    for mut input in &mut inputs {
        if input.input == Vec2::ZERO {
            continue;
        }

        writer.send(PlayerInput {
            movement: input.input,
        });

        input.input = Vec2::ZERO;
    }
}
fn cancel_mouse(mut inputs: Query<&mut AccumulatedInputs, With<LocalPlayer>>) {
    for mut input in &mut inputs {
        input.input = Vec2::ZERO;
    }
}

fn camera_follow(
    mut camera: Query<&mut Transform, With<Camera3d>>,
    player: Query<&Transform, (With<Player>, With<LocalPlayer>, Without<Camera3d>)>,
) {
    let Ok(mut camera_transform) = camera.get_single_mut() else {
        return;
    };

    let Ok(player_transform) = player.get_single() else {
        return;
    };

    camera_transform.look_at(player_transform.translation, Vec3::Y);
}

fn scroll_events(
    mut camera: Query<&mut Transform, With<Camera3d>>,
    mut mouse_scroll_events: EventReader<MouseWheel>,
) {
    for mouse_wheel in mouse_scroll_events.read() {
        let Ok(mut camera_transform) = camera.get_single_mut() else {
            continue;
        };

        camera_transform.translation.y += 1.0 * mouse_wheel.y.signum();
    }
}

fn draw_gizmos(
    player_q: Query<&Transform, (With<Player>, With<LocalPlayer>)>,
    input_q: Query<&AccumulatedInputs>,
    mut gizmos: Gizmos,
) {
    let Ok(input) = input_q.get_single() else {
        return;
    };

    let Ok(player_transform) = player_q.get_single() else {
        return;
    };

    if input.input == Vec2::ZERO {
        return;
    }

    let mut end = player_transform.translation.clone();
    end.x += input.x;
    end.z += input.y;

    gizmos.arrow(player_transform.translation, end, RED);
}
