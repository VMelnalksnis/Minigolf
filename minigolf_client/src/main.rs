mod network;
mod ui;

use {
    crate::{network::ClientNetworkPlugin, ui::ClientUiPlugin},
    aeronet::io::{Session, connection::Disconnected},
    bevy::{
        color::palettes::basic::RED,
        input::{
            common_conditions::{input_just_released, input_pressed},
            mouse::{MouseMotion, MouseWheel},
        },
        prelude::*,
    },
    minigolf::{GameState, LevelMesh, MinigolfPlugin, Player, PlayerInput},
    web_sys::{HtmlCanvasElement, wasm_bindgen::JsCast},
};

fn main() -> AppExit {
    App::new()
        .add_plugins((
            DefaultPlugins,
            ClientUiPlugin,
            ClientNetworkPlugin,
            MinigolfPlugin,
        ))
        .add_systems(Startup, setup_level)
        .add_systems(
            Update,
            (
                handle_inputs.run_if(in_state(GameState::Playing)),
                launch_inputs
                    .run_if(in_state(GameState::Playing).and(input_pressed(MouseButton::Left))),
                handle_mouse.run_if(
                    in_state(GameState::Playing).and(input_just_released(MouseButton::Left)),
                ),
                cancel_mouse.run_if(
                    in_state(GameState::Playing).and(input_just_released(MouseButton::Right)),
                ),
                camera_follow.run_if(in_state(GameState::Playing)),
                scroll_events,
                draw_gizmos,
            ),
        )
        .add_observer(on_connected)
        .add_observer(on_player_added)
        .add_observer(on_level_mesh_added)
        .add_observer(on_disconnected)
        .run()
}

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
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn on_player_added(
    trigger: Trigger<OnAdd, Player>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let entity = trigger.entity();
    let player_mesh_handle: Handle<Mesh> = server.load("Player.glb#Mesh0/Primitive0");

    commands.entity(entity).insert((
        AccumulatedInputs { input: Vec2::ZERO },
        Mesh3d(player_mesh_handle.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("#ffd891").unwrap().into(),
            metallic: 0.5,
            perceptual_roughness: 0.5,
            ..default()
        })),
    ));
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

fn launch_inputs(
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut inputs: Query<&mut AccumulatedInputs>,
) {
    for ev in mouse_motion_events.read() {
        let Ok(mut input) = inputs.get_single_mut() else {
            continue;
        };

        input.input += ev.delta / 100.0;
        input.input = input.input.clamp_length_max(1.0);
    }
}

fn handle_mouse(mut writer: EventWriter<PlayerInput>, mut inputs: Query<&mut AccumulatedInputs>) {
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
fn cancel_mouse(mut inputs: Query<&mut AccumulatedInputs>) {
    for mut input in &mut inputs {
        input.input = Vec2::ZERO;
    }
}

fn camera_follow(
    mut camera: Query<&mut Transform, With<Camera3d>>,
    player: Query<&Transform, (With<Player>, Without<Camera3d>)>,
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
        info!("{mouse_wheel:?}");

        let Ok(mut camera_transform) = camera.get_single_mut() else {
            continue;
        };

        camera_transform.translation.y += 1.0 * mouse_wheel.y.signum();
    }
}

fn draw_gizmos(
    player_q: Query<&Transform, With<Player>>,
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
