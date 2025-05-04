use {
    crate::{Configuration, GameLayer, ServerState, course::PlayingSet},
    avian3d::{math::Scalar, prelude::*},
    bevy::{app::App, ecs::entity::EntityHashSet, math::DVec3, prelude::*},
    minigolf::Player,
};

pub(crate) struct CourseEntitiesPlugin;

impl Plugin for CourseEntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Bumper>();
        app.register_type::<JumpPad>();
        app.register_type::<BallMagnet>();

        app.add_systems(OnEnter(ServerState::WaitingForPlayers), setup);

        app.add_systems(Update, add_required_ball_magnet_components); // todo
        app.add_systems(
            Update,
            (despawn_bumpers, apply_ball_magnet).in_set(PlayingSet),
        );
    }
}

fn setup(mut commands: Commands) {
    commands.spawn_batch([
        (
            Name::new("Bumper collision observer"),
            StateScoped(ServerState::Playing),
            Observer::new(apply_bumper_impulse),
        ),
        (
            Name::new("Jump pad collision observer"),
            StateScoped(ServerState::Playing),
            Observer::new(apply_jump_pad_impulse),
        ),
    ]);
}

/// Component for identifying bumper entities.
#[derive(Component, Reflect, Debug)]
#[require(
    RigidBody::Static,
    CollisionEventsEnabled,
    CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
    ColliderConstructor::Cylinder{ radius: 0.042672, height: 0.05 })]
pub(crate) struct Bumper {
    hits: Option<usize>,
}

impl Bumper {
    pub(crate) fn permanent() -> Self {
        Self { hits: None }
    }

    pub(crate) fn with_hits(hits: usize) -> Self {
        Self { hits: Some(hits) }
    }
}

fn apply_bumper_impulse(
    trigger: Trigger<OnCollisionStart>,
    mut bumpers: Query<(&Position, &mut Bumper)>,
    players: Query<&Position, With<Player>>,
    mut commands: Commands,
    config: Res<Configuration>,
) {
    let bumper_entity = trigger.target();
    let Ok((bumper_position, mut bumper)) = bumpers.get_mut(bumper_entity) else {
        return;
    };

    let other_entity = trigger.0;
    let Ok(player_position) = players.get(other_entity) else {
        return;
    };

    // todo: probably should handle collisions from above differently
    let direction = (player_position.0 - bumper_position.0).normalize();

    info!(
        "Applying bumper effect to player {:?} in direction {:?}",
        other_entity, direction
    );

    commands
        .entity(other_entity)
        .insert(ExternalImpulse::new(direction * config.bumper_strength).with_persistence(false));

    if let Some(current_hits) = bumper.hits {
        bumper.hits = Some(current_hits - 1);
    }
}

fn despawn_bumpers(bumpers: Query<(Entity, &Bumper), Changed<Bumper>>, mut commands: Commands) {
    bumpers
        .into_iter()
        .filter(|(_, bumper)| bumper.hits.is_some_and(|hits| hits <= 0))
        .for_each(|(entity, _)| commands.entity(entity).despawn());
}

/// Component for identifying jump pad entities.
#[derive(Component, Reflect, Debug)]
#[require(
    RigidBody::Static,
    ColliderConstructor::Cylinder{ radius: 0.085344, height: 0.05 },
    Sensor)]
pub(crate) struct JumpPad;

fn apply_jump_pad_impulse(
    trigger: Trigger<OnCollisionStart>,
    jump_pads: Query<(), With<JumpPad>>,
    players: Query<(), With<Player>>,
    mut commands: Commands,
    config: Res<Configuration>,
) {
    let jump_pad_entity = trigger.target();
    let Ok(_) = jump_pads.get(jump_pad_entity) else {
        return;
    };

    let other_entity = trigger.0;
    let Ok(_) = players.get(other_entity) else {
        return;
    };

    // todo: can get stuck on jump pads when entering without enough horizontal velocity
    let direction = DVec3::Y;

    info!(
        "Applying jump pad effect to player {:?} in direction {:?}",
        other_entity, direction
    );

    commands
        .entity(other_entity)
        .insert(ExternalImpulse::new(direction * config.jump_pad_strength).with_persistence(false));
}

#[derive(Component, Reflect, Debug)]
#[require(
    RigidBody::Static,
    CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
    CollidingEntities,
    Sensor)]
pub(crate) struct BallMagnet {
    max_distance: Scalar,
    strength: f32,
}

impl Default for BallMagnet {
    fn default() -> Self {
        BallMagnet {
            max_distance: 0.2,
            strength: 0.0125,
        }
    }
}

fn add_required_ball_magnet_components(
    magnets: Query<(Entity, &BallMagnet), Added<BallMagnet>>,
    mut commands: Commands,
) {
    magnets.iter().for_each(|(entity, ball_magnet)| {
        commands.entity(entity).insert(ColliderConstructor::Sphere {
            radius: ball_magnet.max_distance.to_owned(),
        });
    });
}

fn apply_ball_magnet(
    magnets: Query<(Entity, &BallMagnet, &CollidingEntities)>,
    mut players: Query<(Entity, Option<&mut ExternalImpulse>), With<Player>>,
    transforms: Query<&GlobalTransform>,
    mut commands: Commands,
) {
    let players_hash_set = EntityHashSet::from_iter(players.iter().map(|(e, _)| e));

    for (magnet_entity, ball_magnet, colliding_entities) in magnets.iter() {
        let magnet_transform = transforms.get(magnet_entity).unwrap();

        for player in colliding_entities.intersection(&players_hash_set) {
            let (player, existing_impulse) = players.get_mut(*player).unwrap();

            let player_transform = transforms.get(player).unwrap();
            let vector = magnet_transform.translation() - player_transform.translation();
            let normalized = vector.normalize() * ball_magnet.strength;

            // todo: test if this is needed - added this because of a suspicion that this might not work together with wind
            if let Some(mut impulse) = existing_impulse {
                impulse.apply_impulse(normalized.into());
            } else {
                let impulse = ExternalImpulse::new(normalized.into()).with_persistence(false);
                commands.entity(player).insert(impulse);
            }
        }
    }
}
