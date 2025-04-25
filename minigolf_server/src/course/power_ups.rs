use {
    crate::{
        ServerState,
        course::{CurrentHole, HoleCompleted, HoleWalls, PlayingSet},
        server::{LastPlayerPosition, ValidPlayerInput},
    },
    avian3d::prelude::*,
    bevy::prelude::*,
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
                apply_sticky,
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
}

const WIND_POWER_UP_STRENGTH: f32 = 0.3;

fn apply_power_ups(
    mut reader: EventReader<ValidPlayerInput>,
    current_hole: Res<CurrentHole>,
    mut commands: Commands,
    players: Query<Entity, With<Player>>,
    hole_walls: Query<(Entity, &HoleWalls)>,
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
                let force = Vec3::new(direction.x, 0.0, direction.y) * WIND_POWER_UP_STRENGTH;

                for player in players.iter() {
                    commands.entity(player).insert(ExternalForce::new(force));
                }
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

const HOLE_MAGNET_MAX_DISTANCE: f32 = 0.2;
const HOLE_MAGNET_MIN_DISTANCE: f32 = 0.05;

fn apply_hole_magnet(
    current_hole: Res<CurrentHole>,
    mut commands: Commands,
    transforms: Query<&GlobalTransform>,
    players: Query<(Entity, &GlobalTransform), (With<Player>, With<HoleMagnetPowerUp>)>,
    time: Res<Time<Fixed>>,
) {
    let hole_transform = transforms.get(current_hole.hole_entity).unwrap();

    for (player, transform) in players.iter() {
        let vector = hole_transform.translation() - transform.translation();
        let distance = vector.length();

        if distance >= HOLE_MAGNET_MAX_DISTANCE || distance <= HOLE_MAGNET_MIN_DISTANCE {
            continue;
        }

        let force = vector.normalize() * time.delta_secs() * 50.0;
        commands
            .entity(player)
            .insert(ExternalForce::new(force).with_persistence(false));
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

fn apply_sticky(
    collisions: Collisions,
    walls: Query<Entity, With<HoleWalls>>,
    sticky_walls: Query<(), (With<HoleWalls>, With<StickyWalls>)>,
    players: Query<(Entity, &Player)>,
    sticky_players: Query<(), (With<Player>, With<StickyBall>)>,
    mut velocities: Query<(&mut LinearVelocity, &mut AngularVelocity)>,
) {
    let walls = walls.iter().collect::<Vec<_>>();

    // todo: broken in latest avian version
    for contact_pair in collisions.iter().filter(|c| !c.is_sensor()) {
        let Some(player_entity) = find_entity(
            &players.iter().map(|(e, _)| e).collect::<Vec<_>>(),
            contact_pair,
        ) else {
            continue;
        };

        let Some(walls_entity) = find_entity(&walls, contact_pair) else {
            continue;
        };

        if sticky_walls.get(walls_entity).is_err() && sticky_players.get(player_entity).is_err() {
            continue;
        }

        let Ok((_, player)) = players.get(player_entity) else {
            continue;
        };

        if player.can_move {
            continue;
        }

        info!(
            "Applying sticky effect for player {:?}, walls {:?} with contacts {:?}",
            player_entity, walls_entity, contact_pair
        );

        let (mut linear, mut angular) = velocities.get_mut(player_entity).unwrap();
        linear.0 = Vec3::ZERO;
        angular.0 = Vec3::ZERO;
    }
}

fn find_entity(entities: &Vec<Entity>, contacts: &ContactPair) -> Option<Entity> {
    if entities.contains(&contacts.entity1) {
        Some(contacts.entity1)
    } else if entities.contains(&contacts.entity2) {
        Some(contacts.entity2)
    } else {
        None
    }
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
