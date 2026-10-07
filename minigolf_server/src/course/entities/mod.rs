use {
    crate::{Configuration, GameLayer, PlayingSystems, ServerState},
    avian3d::{
        math::{Scalar, Vector},
        prelude::*,
    },
    bevy::{app::App, prelude::*},
    minigolf::{Attractor, Player},
};

pub(crate) struct CourseEntitiesPlugin;

impl Plugin for CourseEntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Bumper>();
        app.register_type::<JumpPad>();
        app.register_type::<AttractsOnly>();

        app.add_systems(OnEnter(ServerState::Playing), setup);

        app.add_systems(Update, despawn_bumpers.in_set(PlayingSystems));
        app.add_systems(FixedUpdate, apply_attractors.in_set(PlayingSystems));
    }
}

fn setup(mut commands: Commands) {
    commands.spawn_batch([
        (
            Name::new("Bumper collision observer"),
            DespawnOnExit(ServerState::Playing),
            Observer::new(apply_bumper_impulse),
        ),
        (
            Name::new("Jump pad collision observer"),
            DespawnOnExit(ServerState::Playing),
            Observer::new(apply_jump_pad_impulse),
        ),
    ]);
}

pub(crate) const BUMPER_RADIUS: Scalar = 0.042672;
pub(crate) const BUMPER_HEIGHT: Scalar = 0.05;

/// Component for identifying bumper entities.
#[derive(Component, Reflect, Debug)]
#[require(
    RigidBody::Static,
    CollisionEventsEnabled,
    CollisionLayers::new(GameLayer::Default, [GameLayer::Player]),
    ColliderConstructor::Cylinder{ radius: BUMPER_RADIUS, height: BUMPER_HEIGHT })]
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
    trigger: On<CollisionStart>,
    mut bumpers: Query<(&Position, &mut Bumper)>,
    players: Query<&Position, With<Player>>,
    mut forces : Query<Forces>,
    config: Res<Configuration>,
) {
    let bumper_entity = trigger.collider1;
    let Ok((bumper_position, mut bumper)) = bumpers.get_mut(bumper_entity) else {
        return;
    };

    let other_entity = trigger.collider2;
    let Ok(player_position) = players.get(other_entity) else {
        return;
    };

    // todo: probably should handle collisions from above differently
    let direction = (player_position.0 - bumper_position.0).normalize();

    info!(
        "Applying bumper effect to player {:?} in direction {:?}",
        other_entity, direction
    );

    forces
        .get_mut(other_entity)
        .unwrap()
        .apply_linear_impulse(direction * config.bumper_strength);

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
    trigger: On<CollisionStart>,
    jump_pads: Query<(), With<JumpPad>>,
    players: Query<(), With<Player>>,
    mut forces: Query<Forces>,
    config: Res<Configuration>,
) {
    let jump_pad_entity = trigger.collider1;
    let Ok(_) = jump_pads.get(jump_pad_entity) else {
        return;
    };

    let other_entity = trigger.collider2;
    let Ok(_) = players.get(other_entity) else {
        return;
    };

    // todo: can get stuck on jump pads when entering without enough horizontal velocity
    let direction = Vector::Y;

    info!(
        "Applying jump pad effect to player {:?} in direction {:?}",
        other_entity, direction
    );

    forces
        .get_mut(other_entity)
        .unwrap()
        .apply_linear_impulse(direction * config.jump_pad_strength);
}

/// Limits an [Attractor] to pull only the given ball.
#[derive(Component, Reflect, Debug)]
pub(crate) struct AttractsOnly(pub(crate) Entity);

pub(crate) fn apply_attractors(
    attractors: Query<(&Attractor, &GlobalTransform, Option<&AttractsOnly>)>,
    players: Query<(Entity, &GlobalTransform), With<Player>>,
    mut forces: Query<Forces>,
) {
    for (attractor, attractor_transform, attracts_only) in &attractors {
        for (player, player_transform) in &players {
            if attracts_only.is_some_and(|only| only.0 != player) {
                continue;
            }

            // Only pull along the floor, the center can be below or above the ball.
            let vector =
                (attractor_transform.translation() - player_transform.translation()).with_y(0.0);
            let distance = vector.length();

            if distance >= attractor.radius || distance <= attractor.min_radius {
                continue;
            }

            let force = vector / distance * attractor.strength;
            forces
                .get_mut(player)
                .unwrap()
                .apply_force(Vector::from(force));
        }
    }
}
