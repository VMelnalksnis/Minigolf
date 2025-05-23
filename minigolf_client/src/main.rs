mod input;
mod network;
mod ui;

use {
    crate::{
        input::{AccumulatedInputs, MinigolfInputPlugin, camera::TargetTransform},
        network::{Authentication, ClientNetworkPlugin},
        ui::{ClientUiPlugin, ServerState},
    },
    aeronet::io::{Session, connection::Disconnected},
    bevy::{
        ecs::query::QuerySingleError,
        pbr::{DirectionalLightShadowMap, ShadowFilteringMethod},
        prelude::{AlphaMode::Blend, *},
        window::PrimaryWindow,
    },
    bevy_replicon::prelude::*,
    minigolf::{GameState, LevelMesh, MinigolfPlugin, Player, PowerUp},
    web_sys::{HtmlCanvasElement, wasm_bindgen::JsCast},
};

fn main() -> AppExit {
    App::new()
        .register_type::<LocalPlayer>()
        .add_plugins((
            DefaultPlugins,
            ClientUiPlugin,
            ClientNetworkPlugin,
            MinigolfPlugin,
            MinigolfInputPlugin,
        ))
        .register_required_components::<Children, InheritedVisibility>()
        .add_systems(Startup, (set_window_title, setup_level))
        .add_observer(on_connected)
        .add_observer(on_player_added)
        .add_observer(on_level_mesh_added)
        .add_observer(on_power_up_added)
        .add_observer(on_disconnected)
        .add_systems(OnExit(ServerState::GameServer), despawn_replicated)
        .run()
}

fn set_window_title(mut primary_windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = primary_windows.single_mut() {
        window.title = "Minigolf".to_string();
    }
}

#[derive(Component, Reflect, Debug)]
struct LocalPlayer;

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

    commands.spawn((
        DirectionalLight {
            illuminance: 1000.0,
            shadows_enabled: true,
            shadow_depth_bias: 0.005,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -45.0, 0.0, -45.0)),
    ));

    commands.insert_resource::<DirectionalLightShadowMap>(DirectionalLightShadowMap { size: 4096 });

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 5.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        TargetTransform::new(Transform::from_xyz(-2.5, 5.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y)),
        Msaa::Sample4, // WebGPU is only guaranteed to support 4
        ShadowFilteringMethod::Gaussian,
        MeshPickingCamera,
    ));
}

fn on_level_mesh_added(
    trigger: Trigger<OnAdd, LevelMesh>,
    query: Query<&LevelMesh>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let entity = trigger.target();
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

fn on_power_up_added(
    trigger: Trigger<OnAdd, PowerUp>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let entity = trigger.target();

    commands.entity(entity).insert((
        Mesh3d(meshes.add(Sphere::new(0.1))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.3, 0.3, 0.7, 0.5),
            alpha_mode: Blend,
            emissive: LinearRgba::BLUE,
            ..default()
        })),
        PointLight {
            intensity: 2000.0,
            range: 20.0,
            color: Color::srgb(0.3, 0.3, 0.7),
            radius: 0.1,
            shadows_enabled: true,
            ..default()
        },
    ));
}

fn on_connected(_trigger: Trigger<OnAdd, Session>, mut game_state: ResMut<NextState<GameState>>) {
    game_state.set(GameState::Playing);
}

fn on_disconnected(_trigger: Trigger<Disconnected>, mut game_state: ResMut<NextState<GameState>>) {
    game_state.set(GameState::None);
}

fn on_player_added(
    trigger: Trigger<OnAdd, Player>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    players: Query<(), With<LocalPlayer>>,
    all_players: Query<(Entity, &Player)>,
    authentication: Res<Authentication>,
) {
    let entity = trigger.target();
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

    if let Err(QuerySingleError::NoEntities(_)) = players.single() {
        let x = all_players
            .iter()
            .filter(|(e, p)| *e == entity && p.id == authentication.id)
            .map(|(e, _)| e)
            .collect::<Vec<_>>();

        if let &[_] = x.as_slice() {
            commands
                .entity(entity)
                .insert((LocalPlayer, AccumulatedInputs::default()));
        }
    }
}

/// Just to be safe that all entities from the server are removed
fn despawn_replicated(replicated: Query<Entity, With<Replicated>>, mut commands: Commands) {
    for entity in replicated.iter() {
        commands.entity(entity).despawn();
    }
}
