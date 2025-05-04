use {
    crate::{LocalPlayer, input::InputTarget, ui::ServerState},
    bevy::prelude::*,
    bevy_egui::{EguiContexts, egui},
    minigolf::{Player, PlayerInput, PlayerPowerUps, PlayerScore, PowerUpType::*},
};

/// UI for displaying and interacting with power ups
pub(crate) struct PowerUpUiPlugin;

impl Plugin for PowerUpUiPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            PowerUpUiSet.run_if(in_state(ServerState::GameServer)),
        )
        .add_systems(Update, (power_up_ui, score_board).in_set(PowerUpUiSet));
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct PowerUpUiSet;

fn score_board(mut context: EguiContexts, scores: Query<(&Player, &PlayerScore)>) {
    egui::Window::new("Scoreboard").show(context.ctx_mut(), |ui| {
        ui.vertical(|ui| {
            for (player, score) in scores {
                ui.horizontal(|ui| {
                    ui.label(format!("Player \"{:?}\": {:?}", player.id, score.score));
                });
            }
        })
    });
}

fn power_up_ui(
    mut context: EguiContexts,
    player: Query<&PlayerPowerUps, With<LocalPlayer>>,
    mut writer: EventWriter<PlayerInput>,
    mut input_target: ResMut<NextState<InputTarget>>,
) {
    let Ok(power_ups) = player.single() else {
        return;
    };

    egui::Window::new("Power ups").show(context.ctx_mut(), |ui| {
        ui.vertical(|ui| {
            for power_up_type in power_ups.get_power_ups() {
                ui.horizontal(|ui| {
                    ui.label(format!("{:?}", power_up_type));

                    if ui.button("Use").clicked() {
                        info!("Use power up {:?}", power_up_type);

                        match power_up_type {
                            Teleport => {
                                input_target.set(InputTarget::Teleport);
                            }

                            ChipShot => {
                                writer.write(PlayerInput::ChipShot);
                            }

                            HoleMagnet => {
                                writer.write(PlayerInput::HoleMagnet);
                            }

                            StickyBall => {
                                writer.write(PlayerInput::StickyBall);
                            }

                            Bumper => {
                                input_target.set(InputTarget::Bumper);
                            }

                            BlackHoleBumper => {
                                input_target.set(InputTarget::BlackHoleBumper);
                            }

                            Wind => {
                                writer.write(PlayerInput::Wind(Vec2::new(1.0, 1.0))); // todo
                            }

                            StickyWalls => {
                                writer.write(PlayerInput::StickyWalls);
                            }

                            IceRink => {
                                writer.write(PlayerInput::IceRink);
                            }

                            _ => {}
                        };
                    }
                });
            }
        })
    });
}
