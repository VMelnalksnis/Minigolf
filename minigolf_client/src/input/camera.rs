use {
    crate::{LocalPlayer, input::InputTarget},
    bevy::{
        app::App,
        input::{mouse::MouseMotion, mouse::MouseWheel},
        math::Vec3,
        prelude::*,
    },
    minigolf::GameState,
    std::f32::consts::PI,
};

pub(crate) struct CameraInputPlugin;

impl Plugin for CameraInputPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TargetTransform>();

        app.configure_sets(Update, CameraInputSet.run_if(in_state(GameState::Playing)));

        app.add_systems(
            Update,
            (
                follow_player_with_camera,
                move_camera_based_on_scroll,
                interpolate_position,
                accumulate_mouse_movement.run_if(in_state(InputTarget::Camera)),
            )
                .in_set(CameraInputSet),
        );
    }
}

#[derive(Component, Reflect, Debug)]
pub(crate) struct TargetTransform {
    target: Vec3,
    distance: f32,
    height: f32,
    rotation: Quat,
}

impl TargetTransform {
    pub(crate) fn new(transform: Transform) -> Self {
        TargetTransform {
            target: transform.translation,
            distance: 2.0,
            height: 1.0,
            rotation: Quat::from_euler(EulerRot::XYZ, 0.0, PI, 0.0),
        }
    }
}

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct CameraInputSet;

fn interpolate_position(mut transforms: Query<(&mut Transform, &TargetTransform)>) {
    for (mut transform, target) in &mut transforms {
        let target_translation = target
            .rotation
            .mul_vec3(Vec3::new(target.distance, target.height, 0.0))
            + target.target;

        transform.translation = transform.translation.lerp(target_translation, 0.05);
        transform.rotation = transform.looking_at(target.target, Vec3::Y).rotation;
    }
}

fn accumulate_mouse_movement(
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut inputs: Query<&mut TargetTransform, With<Camera3d>>,
) {
    for ev in mouse_motion_events.read() {
        let Ok(mut target) = inputs.single_mut() else {
            continue;
        };

        target.rotation *= Quat::from_euler(EulerRot::XYZ, 0.0, ev.delta.x / 100.0 * PI, 0.0);
    }
}

fn follow_player_with_camera(
    player: Query<&Transform, With<LocalPlayer>>,
    mut camera: Query<&mut TargetTransform, With<Camera3d>>,
) {
    let Ok(mut camera) = camera.single_mut() else {
        return;
    };

    match player.single() {
        Ok(position) => camera.target = position.translation,
        _ => {}
    };
}

fn move_camera_based_on_scroll(
    mut camera: Query<&mut TargetTransform, With<Camera3d>>,
    mut mouse_scroll_events: EventReader<MouseWheel>,
) {
    for mouse_wheel in mouse_scroll_events.read() {
        let Ok(mut camera_transform) = camera.single_mut() else {
            continue;
        };

        camera_transform.distance += 0.1 * mouse_wheel.y.signum();
        camera_transform.height += 0.05 * mouse_wheel.y.signum();
    }
}
