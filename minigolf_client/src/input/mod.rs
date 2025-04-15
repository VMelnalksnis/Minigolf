use {
    crate::{LocalPlayer, input::camera::CameraInputPlugin},
    bevy::{
        app::App,
        input::{
            common_conditions::{input_just_released, input_pressed},
            mouse::MouseMotion,
            touch::TouchPhase,
        },
        prelude::*,
    },
    minigolf::{GameState, Player, PlayerInput},
};

mod camera;

pub(crate) struct MinigolfInputPlugin;

impl Plugin for MinigolfInputPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<CameraInputPlugin>() {
            app.add_plugins(CameraInputPlugin);
        }

        app.register_type::<AccumulatedInputs>();

        app.configure_sets(
            Update,
            ValidateInputSet.run_if(in_state(GameState::Playing)),
        );

        app.add_systems(Update, check_whether_can_move.in_set(ValidateInputSet));

        app.init_state::<InputState>().init_resource::<TouchState>();

        app.configure_sets(
            Update,
            InputSet
                .run_if(in_state(GameState::Playing).and(in_state(InputState::CanMove)))
                .after(ValidateInputSet),
        );

        app.add_systems(
            Update,
            (
                accumulate_mouse_movement.run_if(input_pressed(MouseButton::Left)),
                send_inputs_to_server.run_if(input_just_released(MouseButton::Left)),
                reset_inputs.run_if(input_just_released(MouseButton::Right)),
                handle_touch,
                draw_accumulated_inputs,
            )
                .in_set(InputSet),
        );
    }
}

#[derive(States, Reflect, Default, Debug, Clone, PartialEq, Eq, Hash)]
enum InputState {
    #[default]
    CanMove,
    CannotMove,
}

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ValidateInputSet;

fn check_whether_can_move(
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

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct InputSet;

#[derive(Component, Reflect, Deref, DerefMut, Default, Debug)]
pub(crate) struct AccumulatedInputs {
    input: Vec2,
}

fn accumulate_mouse_movement(
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut inputs: Query<&mut AccumulatedInputs, With<LocalPlayer>>,
) {
    for ev in mouse_motion_events.read() {
        let Ok(mut input) = inputs.get_single_mut() else {
            continue;
        };

        input.input.y -= ev.delta.x / 400.0;
        input.input.x += ev.delta.y / 400.0;

        input.input = input.input.clamp_length_max(1.0);
    }
}

fn send_inputs_to_server(
    mut writer: EventWriter<PlayerInput>,
    mut inputs: Query<&mut AccumulatedInputs, With<LocalPlayer>>,
) {
    let Ok(mut input) = inputs.get_single_mut() else {
        error!("Multiple entities with accumulated inputs/local player marker ");
        return;
    };

    if input.input == Vec2::ZERO {
        return;
    }

    writer.send(PlayerInput {
        movement: input.input,
    });

    input.input = Vec2::ZERO;
}

fn reset_inputs(mut inputs: Query<&mut AccumulatedInputs, With<LocalPlayer>>) {
    let Ok(mut input) = inputs.get_single_mut() else {
        error!("Multiple entities with accumulated inputs/local player marker ");
        return;
    };

    input.input = Vec2::ZERO;
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

fn draw_accumulated_inputs(
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
    end.x += input.x * 2.0;
    end.z += input.y * 2.0;

    gizmos.arrow(
        player_transform.translation,
        end,
        bevy::color::palettes::basic::RED,
    );
}
