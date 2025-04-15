use {
    crate::LocalPlayer,
    bevy::{app::App, input::mouse::MouseWheel, math::Vec3, prelude::*},
    minigolf::{GameState, Player},
};

pub(crate) struct CameraInputPlugin;

impl Plugin for CameraInputPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, CameraInputSet.run_if(in_state(GameState::Playing)));

        app.add_systems(
            Update,
            (
                follow_player_with_camera,
                look_at_player_with_camera,
                move_camera_based_on_scroll,
            )
                .in_set(CameraInputSet),
        );
    }
}

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct CameraInputSet;

fn follow_player_with_camera(
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

    camera.translation.x = position - 1.0;
}

fn look_at_player_with_camera(
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

fn move_camera_based_on_scroll(
    mut camera: Query<&mut Transform, With<Camera3d>>,
    mut mouse_scroll_events: EventReader<MouseWheel>,
) {
    for mouse_wheel in mouse_scroll_events.read() {
        let Ok(mut camera_transform) = camera.get_single_mut() else {
            continue;
        };

        camera_transform.translation.y += 0.1 * mouse_wheel.y.signum();
    }
}
