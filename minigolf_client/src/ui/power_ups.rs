use minigolf::{PlayerInput, PowerUpType};
use {
    crate::{LocalPlayer, ui::ServerState},
    bevy::prelude::*,
    bevy_egui::{EguiContexts, egui},
    minigolf::PlayerPowerUps,
};

/// UI for displaying and interacting with power ups
pub(crate) struct PowerUpUiPlugin;

impl Plugin for PowerUpUiPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            PowerUpUiSet.run_if(in_state(ServerState::GameServer)),
        )
        .add_systems(Update, power_up_ui.in_set(PowerUpUiSet));
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct PowerUpUiSet;

fn power_up_ui(
    mut context: EguiContexts,
    player: Query<&PlayerPowerUps, With<LocalPlayer>>,
    mut writer: EventWriter<PlayerInput>,
) {
    let Ok(power_ups) = player.get_single() else {
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
                            PowerUpType::HoleMagnet => {
                                writer.send(PlayerInput::HoleMagnet);
                            }
                            
                            PowerUpType::StickyBall => {
                                writer.send(PlayerInput::StickyBall);
                            }

                            PowerUpType::Wind => {
                                writer.send(PlayerInput::Wind(Vec2::new(1.0, 1.0))); // todo
                            }

                            PowerUpType::StickyWalls => {
                                writer.send(PlayerInput::StickyWalls);
                            }

                            PowerUpType::IceRink => {
                                writer.send(PlayerInput::IceRink);
                            }

                            _ => {}
                        };
                    }
                });
            }
        })
    });
}
