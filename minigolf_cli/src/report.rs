use {
    crate::{network::Authentication, output},
    bevy::{
        ecs::system::SystemParam, platform::collections::HashMap, prelude::*,
        transform::TransformSystems,
    },
    bevy_replicon::prelude::*,
    minigolf::{
        Attractor, BallCollision, CourseEffect, LevelMesh, Player, PlayerPowerUps, PlayerScore,
        PowerUp,
    },
};

/// Prints what happens in the game.
pub(crate) struct ReportPlugin;

impl Plugin for ReportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BallMotion>();

        app.add_observer(report_despawned);

        // After transforms are propagated, so that world positions are up to date.
        app.add_systems(
            PostUpdate,
            (
                report_spawned,
                report_changes,
                report_ball_motion,
                report_collisions,
            )
                .chain()
                .after(TransformSystems::Propagate),
        );
    }
}

/// Describes replicated entities for the output.
#[derive(SystemParam)]
pub(crate) struct Describe<'w, 's> {
    names: Query<'w, 's, &'static Name>,
    transforms: Query<'w, 's, &'static GlobalTransform>,
    parents: Query<'w, 's, &'static ChildOf>,
    players: Query<'w, 's, &'static Player>,
    power_ups: Query<'w, 's, &'static PlayerPowerUps>,
    scores: Query<'w, 's, &'static PlayerScore>,
    pickups: Query<'w, 's, &'static PowerUp>,
    attractors: Query<'w, 's, &'static Attractor>,
    effects: Query<'w, 's, &'static CourseEffect>,
    meshes: Query<'w, 's, &'static LevelMesh>,
    authentication: Option<Res<'w, Authentication>>,
}

impl Describe<'_, '_> {
    /// Short label of the entity, e.g. `Hole_0_walls#12` or `Player#20(me)`.
    pub(crate) fn label(&self, entity: Entity) -> String {
        let name = self
            .names
            .get(entity)
            .map(|name| name.as_str().replace(' ', "_"))
            .unwrap_or_else(|_| "?".into());

        let me = match self.is_local_player(entity) {
            true => "(me)",
            false => "",
        };

        format!("{name}#{}{me}", entity.index())
    }

    pub(crate) fn is_local_player(&self, entity: Entity) -> bool {
        let Some(authentication) = &self.authentication else {
            return false;
        };

        self.players
            .get(entity)
            .is_ok_and(|player| player.id == authentication.id)
    }

    pub(crate) fn position(&self, entity: Entity) -> Option<Vec3> {
        self.transforms
            .get(entity)
            .ok()
            .map(GlobalTransform::translation)
    }

    /// Label, world position and the game components of the entity.
    pub(crate) fn describe(&self, entity: Entity) -> String {
        let mut parts = vec![self.label(entity)];

        if let Some(position) = self.position(entity) {
            parts.push(format!("at={}", vec(position)));
        }
        if let Ok(child_of) = self.parents.get(entity) {
            parts.push(format!("parent={}", self.label(child_of.parent())));
        }
        if let Ok(player) = self.players.get(entity) {
            parts.push(format!("can_move={}", player.can_move));
        }
        if let Ok(power_ups) = self.power_ups.get(entity) {
            parts.push(format!("power_ups={:?}", power_ups.get_power_ups()));
        }
        if let Ok(score) = self.scores.get(entity) {
            parts.push(format!("score={}", score.score));
        }
        if let Ok(pickup) = self.pickups.get(entity) {
            parts.push(format!("power_up={:?}", pickup.power_up));
        }
        if let Ok(attractor) = self.attractors.get(entity) {
            parts.push(attractor_details(attractor));
        }
        if let Ok(effect) = self.effects.get(entity) {
            parts.push(format!("effect={effect:?}"));
        }
        if let Ok(mesh) = self.meshes.get(entity) {
            parts.push(format!("mesh={}", mesh.asset));
        }

        parts.join(" ")
    }
}

fn attractor_details(attractor: &Attractor) -> String {
    format!(
        "attractor(radius={},min_radius={},strength={})",
        attractor.radius, attractor.min_radius, attractor.strength
    )
}

/// Formats a position for the output.
pub(crate) fn vec(value: Vec3) -> String {
    format!("({:.3},{:.3},{:.3})", value.x, value.y, value.z)
}

fn report_spawned(spawned: Query<Entity, Added<Remote>>, describe: Describe) {
    let mut spawned = spawned.iter().collect::<Vec<_>>();
    spawned.sort_by_key(|entity| entity.index());

    for entity in spawned {
        output!("spawn {}", describe.describe(entity));
    }
}

fn report_despawned(remove: On<Remove, Remote>, describe: Describe) {
    output!("despawn {}", describe.label(remove.entity));
}

fn report_changes(
    players: Query<(Entity, Ref<Player>, Ref<Remote>)>,
    power_ups: Query<(Entity, Ref<PlayerPowerUps>, Ref<Remote>)>,
    scores: Query<(Entity, Ref<PlayerScore>, Ref<Remote>)>,
    attractors: Query<(Entity, Ref<Attractor>, Ref<Remote>)>,
    effects: Query<(Entity, Ref<CourseEffect>, Ref<Remote>)>,
    mut removed_effects: RemovedComponents<CourseEffect>,
    mut last_values: Local<HashMap<(Entity, &'static str), String>>,
    describe: Describe,
) {
    // The same value can be received more than once, only print actual changes.
    let mut report = |entity: Entity, kind: &'static str, value: String, remote: &Ref<Remote>| {
        let previous = last_values.insert((entity, kind), value.clone());

        // Values of newly spawned entities are part of the spawn output.
        if !remote.is_added() && previous.as_ref() != Some(&value) {
            output!("{kind} {} {value}", describe.label(entity));
        }
    };

    for (entity, player, remote) in &players {
        if player.is_changed() {
            report(entity, "can-move", player.can_move.to_string(), &remote);
        }
    }

    for (entity, power_ups, remote) in &power_ups {
        if power_ups.is_changed() {
            let value = format!("{:?}", power_ups.get_power_ups());
            report(entity, "power-ups", value, &remote);
        }
    }

    for (entity, score, remote) in &scores {
        if score.is_changed() {
            report(entity, "score", score.score.to_string(), &remote);
        }
    }

    for (entity, attractor, remote) in &attractors {
        if attractor.is_changed() {
            report(entity, "attractor", attractor_details(&attractor), &remote);
        }
    }

    for (entity, effect, remote) in &effects {
        if effect.is_changed() {
            report(entity, "effect", format!("{:?}", *effect), &remote);
        }
    }

    for entity in removed_effects.read() {
        last_values.remove(&(entity, "effect"));
        output!("effect-removed {}", describe.label(entity));
    }
}

/// Distance (m) a ball has to move to count as moving.
const MOTION_THRESHOLD: f32 = 0.0005;
/// Time (s) a ball has to stay in place to count as resting.
const REST_DELAY: f32 = 0.3;

/// Tracks whether balls are moving, based on their replicated positions.
#[derive(Resource, Default, Debug)]
pub(crate) struct BallMotion {
    balls: HashMap<Entity, Motion>,
}

#[derive(Debug)]
struct Motion {
    /// Position when the ball last moved.
    position: Vec3,
    last_moved: f32,
    moving: bool,
}

impl BallMotion {
    pub(crate) fn is_moving(&self, ball: Entity) -> bool {
        self.balls.get(&ball).is_some_and(|motion| motion.moving)
    }
}

fn report_ball_motion(
    balls: Query<(Entity, &GlobalTransform), With<Player>>,
    mut motion: ResMut<BallMotion>,
    describe: Describe,
) {
    let now = crate::elapsed();

    for (ball, transform) in &balls {
        let position = transform.translation();
        let motion = motion.balls.entry(ball).or_insert(Motion {
            position,
            last_moved: now,
            moving: false,
        });

        if position.distance(motion.position) > MOTION_THRESHOLD {
            if !motion.moving {
                output!("moving {} from={}", describe.label(ball), vec(motion.position));
            }

            motion.position = position;
            motion.last_moved = now;
            motion.moving = true;
        } else if motion.moving && now - motion.last_moved > REST_DELAY {
            output!("rest {} at={}", describe.label(ball), vec(position));
            motion.moving = false;
        }
    }
}

fn report_collisions(mut reader: MessageReader<BallCollision>, describe: Describe) {
    for collision in reader.read() {
        output!(
            "collision {} target={:?} at={} speed={:.3}",
            describe.label(collision.ball),
            collision.target,
            vec(collision.position),
            collision.speed
        );
    }
}

/// Prints all replicated entities.
pub(crate) fn dump(entities: Query<Entity, With<Remote>>, describe: Describe) {
    let mut entities = entities.iter().collect::<Vec<_>>();
    entities.sort_by_key(|entity| entity.index());

    output!("dump {} entities", entities.len());
    for entity in entities {
        output!("  {}", describe.describe(entity));
    }
}
