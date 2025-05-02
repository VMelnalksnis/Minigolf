use {
    crate::{
        ServerState,
        course::{Configuration, CurrentHole, HoleCompleted, HoleWalls, PlayingSet},
        server::{LastPlayerPosition, ValidPlayerInput},
    },
    avian3d::prelude::*,
    bevy::{math::DVec3, prelude::*},
    minigolf::{Player, PlayerInput, PlayerPowerUps, PowerUp},
};

pub(crate) struct PowerUpPlugin;

impl Plugin for PowerUpPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<HoleMagnetPowerUp>();
        app.register_type::<StickyWalls>();
        app.register_type::<StickyBall>();

        app.add_systems(OnEnter(ServerState::Playing), setup_observers);

        app.add_systems(Update, apply_power_ups.in_set(PlayingSet));

        app.add_systems(
            FixedUpdate,
            (
                handle_power_up_sensors,
                apply_hole_magnet,
                remove_hole_magnet,
            )
                .in_set(PlayingSet),
        );
    }
}

fn setup_observers(mut commands: Commands) {
    commands.spawn((
        Name::new("Remove sticky ball observer"),
        StateScoped(ServerState::Playing),
        Observer::new(remove_sticky_ball),
    ));

    commands.spawn((
        Name::new("Apply sticky effects observer"),
        StateScoped(ServerState::Playing),
        Observer::new(on_player_collided),
    ));
}

fn apply_power_ups(
    mut reader: EventReader<ValidPlayerInput>,
    current_hole: Res<CurrentHole>,
    mut commands: Commands,
    players: Query<Entity, With<Player>>,
    hole_walls: Query<(Entity, &HoleWalls)>,
    config: Res<Configuration>,
) {
    for &ValidPlayerInput { input, player } in reader.read() {
        match input {
            PlayerInput::Move(_) => {}

            PlayerInput::HoleMagnet => {
                commands.entity(player).insert(HoleMagnetPowerUp);
            }

            PlayerInput::StickyBall => {
                for other_player in players.iter().filter(|e| *e != player) {
                    commands.entity(other_player).insert(StickyBall);
                }
            }

            PlayerInput::Wind(direction) => {
                let direction = direction.normalize();
                let force = Vec3::new(direction.x, 0.0, direction.y) * config.wind_strength;

                players.iter().for_each(|player| {
                    commands
                        .entity(player)
                        .insert(ExternalForce::new(DVec3::from(force)));
                });
            }

            PlayerInput::StickyWalls => {
                let walls = hole_walls
                    .iter()
                    .filter(|(_, w)| w.hole_entity == current_hole.hole_entity)
                    .map(|(e, _)| e)
                    .next()
                    .unwrap();

                commands.entity(walls).insert(StickyWalls);
            }

            PlayerInput::IceRink => {
                // todo: visual effect
                commands
                    .entity(current_hole.hole_entity)
                    .insert(Friction::new(0.01).with_combine_rule(CoefficientCombine::Min));
            }

            _ => {
                warn!("Unhandled player input type {:?}", input);
            }
        }
    }
}

fn handle_power_up_sensors(
    power_ups: Query<(Entity, &PowerUp, &CollidingEntities), Changed<CollidingEntities>>,
    mut players: Query<(Entity, &mut PlayerPowerUps), With<Player>>,
    mut commands: Commands,
) {
    for (power_up_entity, power_up, collisions) in power_ups.iter() {
        for (player, mut player_power_ups) in &mut players {
            if !collisions.contains(&player) {
                continue;
            }

            info!(
                "Player {:?} collided with power up {:?}",
                player, power_up_entity
            );

            match player_power_ups.add_power_up(power_up.power_up.clone()) {
                Ok(_) => {
                    info!(
                        "Player {:?} picked up power up {:?}",
                        player, power_up_entity
                    );

                    commands.entity(power_up_entity).despawn();
                }
                Err(_) => {
                    info!(
                        "Player {:?} could not pick up power up {:?}",
                        player, power_up_entity
                    );
                }
            }
        }
    }
}

#[derive(Component, Reflect)]
struct HoleMagnetPowerUp;

fn apply_hole_magnet(
    current_hole: Res<CurrentHole>,
    mut commands: Commands,
    transforms: Query<&GlobalTransform>,
    players: Query<(Entity, &GlobalTransform), (With<Player>, With<HoleMagnetPowerUp>)>,
    time: Res<Time<Fixed>>,
    config: Res<Configuration>,
) {
    let Ok(hole_transform) = transforms.get(current_hole.hole_entity) else {
        return;
    };

    for (player, transform) in players.iter() {
        let vector = hole_transform.translation() - transform.translation();
        let distance = vector.length();

        if distance >= config.hole_magnet_max_distance
            || distance <= config.hole_magnet_min_distance
        {
            continue;
        }

        let force = vector.normalize() * time.delta_secs() * config.hole_magnet_strength;
        commands
            .entity(player)
            .insert(ExternalForce::new(DVec3::from(force)).with_persistence(false));
    }
}

fn remove_hole_magnet(
    players: Query<
        Entity,
        (
            With<Player>,
            With<HoleMagnetPowerUp>,
            Changed<LastPlayerPosition>,
        ),
    >,
    mut commands: Commands,
) {
    players.iter().for_each(|player| {
        commands.entity(player).remove::<HoleMagnetPowerUp>();
    });
}

#[derive(Component, Reflect)]
struct StickyWalls;

#[derive(Component, Reflect)]
pub(crate) struct StickyBall;

fn on_player_collided(
    trigger: Trigger<OnCollisionStart>,
    walls: Query<(), With<HoleWalls>>,
    sticky_walls: Query<(), (With<HoleWalls>, With<StickyWalls>)>,
    players: Query<&Player>,
    sticky_players: Query<(), (With<Player>, With<StickyBall>)>,
    mut velocities: Query<(&mut LinearVelocity, &mut AngularVelocity)>,
    mut commands: Commands,
) {
    let player_entity = trigger.target();
    let Ok(player) = players.get(trigger.target()) else {
        return;
    };

    if player.can_move {
        return;
    }

    let other_entity = trigger.0;
    let Ok(_) = walls.get(other_entity) else {
        return;
    };

    if sticky_walls.get(other_entity).is_err() && sticky_players.get(player_entity).is_err() {
        return;
    }

    info!(
        "Applying sticky effect for player {:?}, walls {:?}",
        player_entity, other_entity
    );

    commands.entity(player_entity).insert(Sleeping);
    let (mut linear, mut angular) = velocities.get_mut(player_entity).unwrap();
    linear.0 = DVec3::ZERO;
    angular.0 = DVec3::ZERO;
}

fn remove_sticky_ball(
    _trigger: Trigger<HoleCompleted>,
    players: Query<Entity, With<Player>>,
    mut commands: Commands,
) {
    players.iter().for_each(|entity| {
        commands.entity(entity).remove::<StickyBall>();
    });
}
