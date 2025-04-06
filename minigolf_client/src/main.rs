use {
    aeronet::io::{
        Session, SessionEndpoint,
        connection::{Disconnect, DisconnectReason, Disconnected},
    },
    aeronet_replicon::client::{AeronetRepliconClient, AeronetRepliconClientPlugin},
    aeronet_websocket::client::{WebSocketClient, WebSocketClientPlugin},
    aeronet_webtransport::{
        cert,
        client::{WebTransportClient, WebTransportClientPlugin},
    },
    bevy::{
        color::palettes::basic::RED,
        ecs::query::QuerySingleError,
        input::{
            common_conditions::{input_just_released, input_pressed, input_toggle_active},
            mouse::{MouseMotion, MouseWheel},
        },
        prelude::*,
    },
    bevy_egui::{EguiContexts, EguiPlugin, egui},
    bevy_inspector_egui::quick::WorldInspectorPlugin,
    bevy_replicon::prelude::*,
    minigolf::{GameState, LevelMesh, MinigolfPlugin, Player, PlayerInput},
    web_sys::{HtmlCanvasElement, wasm_bindgen::JsCast},
};

fn main() -> AppExit {
    App::new()
        .add_plugins((
            // core
            DefaultPlugins,
            EguiPlugin,
            WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
            // transport
            WebTransportClientPlugin,
            WebSocketClientPlugin,
            // SessionVisualizerPlugin,
            // replication
            RepliconPlugins,
            AeronetRepliconClientPlugin,
            // game
            MinigolfPlugin,
        ))
        .init_resource::<GlobalUi>()
        .init_resource::<WebTransportUi>()
        .init_resource::<WebSocketUi>()
        .add_systems(Startup, setup_level)
        .add_systems(
            Update,
            (
                (web_transport_ui, web_socket_ui, global_ui).chain(),
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
        .add_observer(on_connecting)
        .add_observer(on_connected)
        .add_observer(on_player_added)
        .add_observer(on_level_mesh_added)
        .add_observer(on_disconnected)
        .run()
}

#[derive(Debug, Default, Resource)]
struct GlobalUi {
    session_id: usize,
    log: Vec<String>,
}

#[derive(Debug, Default, Resource)]
struct WebTransportUi {
    target: String,
    cert_hash: String,
}

#[derive(Debug, Default, Resource)]
struct WebSocketUi {
    target: String,
}

#[derive(Debug, Clone, Component, Deref, DerefMut, Reflect)]
struct AccumulatedInputs {
    input: Vec2,
}

fn setup_level(mut commands: Commands) {
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

fn on_connecting(
    trigger: Trigger<OnAdd, SessionEndpoint>,
    names: Query<&Name>,
    mut ui_state: ResMut<GlobalUi>,
) {
    let entity = trigger.entity();
    let name = names
        .get(entity)
        .expect("our session entity should have a name");
    ui_state.log.push(format!("{name} connecting"));
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

fn on_connected(
    trigger: Trigger<OnAdd, Session>,
    names: Query<&Name>,
    mut ui_state: ResMut<GlobalUi>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let entity = trigger.entity();
    let name = names
        .get(entity)
        .expect("our session entity should have a name");
    ui_state.log.push(format!("{name} connected"));

    game_state.set(GameState::Playing);
}

fn on_disconnected(
    trigger: Trigger<Disconnected>,
    names: Query<&Name>,
    mut ui_state: ResMut<GlobalUi>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let session = trigger.entity();
    let name = names
        .get(session)
        .expect("our session entity should have a name");
    ui_state.log.push(match &trigger.reason {
        DisconnectReason::User(reason) => {
            format!("{name} disconnected by user: {reason}")
        }
        DisconnectReason::Peer(reason) => {
            format!("{name} disconnected by peer: {reason}")
        }
        DisconnectReason::Error(err) => {
            format!("{name} disconnected due to error: {err:?}")
        }
    });
    game_state.set(GameState::None);
}

fn global_ui(
    mut commands: Commands,
    mut egui: EguiContexts,
    global_ui: Res<GlobalUi>,
    sessions: Query<(Entity, &Name, Option<&Session>), With<SessionEndpoint>>,
    replicon_client: Res<RepliconClient>,
) {
    let stats = replicon_client.stats();
    egui::Window::new("Session Log").show(egui.ctx_mut(), |ui| {
        ui.label("Replicon reports:");
        ui.horizontal(|ui| {
            ui.label(match replicon_client.status() {
                RepliconClientStatus::Disconnected => "Disconnected",
                RepliconClientStatus::Connecting => "Connecting",
                RepliconClientStatus::Connected { .. } => "Connected",
            });
            ui.separator();

            ui.label(format!("RTT {:.0}ms", stats.rtt * 1000.0));
            ui.separator();

            ui.label(format!("Pkt Loss {:.1}%", stats.packet_loss * 100.0));
            ui.separator();

            ui.label(format!("Rx {:.0}bps", stats.received_bps));
            ui.separator();

            ui.label(format!("Tx {:.0}bps", stats.sent_bps));
        });
        match sessions.get_single() {
            Ok((session, name, connected)) => {
                if connected.is_some() {
                    ui.label(format!("{name} connected"));
                } else {
                    ui.label(format!("{name} connecting"));
                }

                if ui.button("Disconnect").clicked() {
                    commands.trigger_targets(Disconnect::new("disconnected by user"), session);
                }
            }
            Err(QuerySingleError::NoEntities(_)) => {
                ui.label("No sessions active");
            }
            Err(QuerySingleError::MultipleEntities(_)) => {
                ui.label("Multiple sessions active");
            }
        }

        ui.separator();

        for msg in &global_ui.log {
            ui.label(msg);
        }
    });
}

//
// WebTransport
//

fn web_transport_ui(
    mut commands: Commands,
    mut egui: EguiContexts,
    mut global_ui: ResMut<GlobalUi>,
    mut ui_state: ResMut<WebTransportUi>,
    sessions: Query<(), With<Session>>,
) {
    const DEFAULT_TARGET: &str = "https://remote-dev:25565";

    egui::Window::new("WebTransport").show(egui.ctx_mut(), |ui| {
        if sessions.iter().next().is_some() {
            ui.disable();
        }

        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

        let mut connect = false;
        ui.horizontal(|ui| {
            let connect_resp = ui.add(
                egui::TextEdit::singleline(&mut ui_state.target)
                    .hint_text(format!("{DEFAULT_TARGET} | [enter] to connect")),
            );
            connect |= connect_resp.lost_focus() && enter_pressed;
            connect |= ui.button("Connect").clicked();
        });

        let cert_hash_resp = ui.add(
            egui::TextEdit::singleline(&mut ui_state.cert_hash)
                .hint_text("(optional) certificate hash"),
        );
        connect |= cert_hash_resp.lost_focus() && enter_pressed;

        if connect {
            let mut target = ui_state.target.clone();
            if target.is_empty() {
                DEFAULT_TARGET.clone_into(&mut target);
            }

            let cert_hash = ui_state.cert_hash.clone();
            let config = web_transport_config(cert_hash);

            global_ui.session_id += 1;
            let name = format!("{}. {target}", global_ui.session_id);
            commands
                .spawn((Name::new(name), AeronetRepliconClient))
                .queue(WebTransportClient::connect(config, target));
        }
    });
}

type WebTransportClientConfig = aeronet_webtransport::client::ClientConfig;

#[cfg(target_family = "wasm")]
fn web_transport_config(cert_hash: String) -> WebTransportClientConfig {
    use aeronet_webtransport::xwt_web::{CertificateHash, HashAlgorithm};

    let server_certificate_hashes = match cert::hash_from_b64(&cert_hash) {
        Ok(hash) => vec![CertificateHash {
            algorithm: HashAlgorithm::Sha256,
            value: Vec::from(hash),
        }],
        Err(err) => {
            warn!("Failed to read certificate hash from string: {err:?}");
            Vec::new()
        }
    };

    WebTransportClientConfig {
        server_certificate_hashes,
        ..Default::default()
    }
}

#[cfg(not(target_family = "wasm"))]
fn web_transport_config(cert_hash: String) -> WebTransportClientConfig {
    use {aeronet_webtransport::wtransport::tls::Sha256Digest, core::time::Duration};

    let config = WebTransportClientConfig::builder().with_bind_default();

    let config = if cert_hash.is_empty() {
        warn!("Connecting without certificate validation");
        config.with_no_cert_validation()
    } else {
        match cert::hash_from_b64(&cert_hash) {
            Ok(hash) => config.with_server_certificate_hashes([Sha256Digest::new(hash)]),
            Err(err) => {
                warn!("Failed to read certificate hash from string: {err:?}");
                config.with_server_certificate_hashes([])
            }
        }
    };

    config
        .keep_alive_interval(Some(Duration::from_secs(1)))
        .max_idle_timeout(Some(Duration::from_secs(5)))
        .expect("should be a valid idle timeout")
        .build()
}

//
// WebSocket
//

fn web_socket_ui(
    mut commands: Commands,
    mut egui: EguiContexts,
    mut global_ui: ResMut<GlobalUi>,
    mut ui_state: ResMut<WebSocketUi>,
    sessions: Query<(), With<Session>>,
) {
    const DEFAULT_TARGET: &str = "ws://remote-dev:25566";

    egui::Window::new("WebSocket").show(egui.ctx_mut(), |ui| {
        if sessions.iter().next().is_some() {
            ui.disable();
        }

        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

        let mut connect = false;
        ui.horizontal(|ui| {
            let connect_resp = ui.add(
                egui::TextEdit::singleline(&mut ui_state.target)
                    .hint_text(format!("{DEFAULT_TARGET} | [enter] to connect")),
            );
            connect |= connect_resp.lost_focus() && enter_pressed;
            connect |= ui.button("Connect").clicked();
        });

        if connect {
            let mut target = ui_state.target.clone();
            if target.is_empty() {
                DEFAULT_TARGET.clone_into(&mut target);
            }

            let config = web_socket_config();

            global_ui.session_id += 1;
            let name = format!("{}. {target}", global_ui.session_id);
            commands
                .spawn((Name::new(name), AeronetRepliconClient))
                .queue(WebSocketClient::connect(config, target));
        }
    });
}

type WebSocketClientConfig = aeronet_websocket::client::ClientConfig;

#[cfg(target_family = "wasm")]
fn web_socket_config() -> WebSocketClientConfig {
    WebSocketClientConfig::default()
}

#[cfg(not(target_family = "wasm"))]
fn web_socket_config() -> WebSocketClientConfig {
    WebSocketClientConfig::builder().with_no_cert_validation()
}

//
// game logic
//

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
