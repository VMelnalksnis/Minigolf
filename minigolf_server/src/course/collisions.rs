use {
    crate::{
        HoleState,
        course::{
            Hole, HoleBoundingBox, HoleSensor, HoleWalls,
            entities::{Bumper, JumpPad},
            power_ups::StickyEffectSystems,
        },
    },
    avian3d::{math::Scalar, prelude::*},
    bevy::{ecs::system::SystemParam, prelude::*},
    bevy_replicon::prelude::*,
    minigolf::{BallCollision, CollisionTarget, Player, PowerUp},
};

/// Sends [BallCollision]s to clients.
pub(crate) struct CollisionEventsPlugin;

impl Plugin for CollisionEventsPlugin {
    fn build(&self, app: &mut App) {
        // Runs after the solver, so that only contacts that pushed the ball back are impacts.
        // Checked every step instead of on `CollisionStart`, because all walls of a hole are a
        // single collider, so hitting a wall while already touching another one does not start a
        // new collision.
        app.add_systems(
            PhysicsSchedule,
            send_impacts
                .in_set(PhysicsStepSystems::Finalize)
                // Both use `Collisions`, which has mutable access to the contact graph.
                .after(StickyEffectSystems)
                .run_if(in_state(HoleState::Playing)),
        );

        app.add_observer(send_sensor_entries);
    }
}

/// Minimum speed (m/s) towards a surface at which a contact counts as an impact.
const MIN_IMPACT_SPEED: Scalar = 0.1;

#[derive(SystemParam)]
struct CollisionTargets<'w, 's> {
    floors: Query<'w, 's, (), With<Hole>>,
    walls: Query<'w, 's, (), With<HoleWalls>>,
    bumpers: Query<'w, 's, (), With<Bumper>>,
    balls: Query<'w, 's, (), With<Player>>,
    jump_pads: Query<'w, 's, (), With<JumpPad>>,
    hole_sensors: Query<'w, 's, (), With<HoleSensor>>,
    power_ups: Query<'w, 's, (), With<PowerUp>>,
    bounds: Query<'w, 's, (), With<HoleBoundingBox>>,
}

impl CollisionTargets<'_, '_> {
    /// Gets what the entity is, or `None` if collisions with it should not be sent.
    fn get(&self, entity: Entity) -> Option<CollisionTarget> {
        let target = if self.floors.contains(entity) {
            CollisionTarget::Floor
        } else if self.walls.contains(entity) {
            CollisionTarget::Wall
        } else if self.bumpers.contains(entity) {
            CollisionTarget::Bumper
        } else if self.balls.contains(entity) {
            CollisionTarget::Ball
        } else if self.jump_pads.contains(entity) {
            CollisionTarget::JumpPad
        } else if self.hole_sensors.contains(entity) {
            CollisionTarget::Hole
        } else if self.power_ups.contains(entity) {
            CollisionTarget::PowerUp
        } else if self.bounds.contains(entity) {
            return None;
        } else {
            CollisionTarget::Other
        };

        Some(target)
    }
}

fn send_impacts(
    balls: Query<Entity, (With<Player>, Without<Sleeping>)>,
    collisions: Collisions,
    targets: CollisionTargets,
    mut writer: MessageWriter<ToClients<BallCollision>>,
) {
    for ball in &balls {
        for pair in collisions.collisions_with(ball) {
            let other = match pair.collider1 == ball {
                true => pair.collider2,
                false => pair.collider1,
            };

            // Collisions between two moving balls are found for both, send them only once.
            if balls.contains(other) && other < ball {
                continue;
            }

            let Some(target) = targets.get(other) else {
                continue;
            };

            // Contact points also include speculative contacts for surfaces the ball might reach,
            // so only count the ones that actually pushed the ball back during this step.
            let impact = pair
                .manifolds
                .iter()
                .flat_map(|manifold| manifold.points.iter())
                .filter(|point| point.normal_impulse > 0.0)
                .min_by(|a, b| a.normal_speed.total_cmp(&b.normal_speed));

            let Some(impact) = impact else {
                continue;
            };

            if impact.normal_speed > -MIN_IMPACT_SPEED {
                continue;
            }

            writer.write(ToClients {
                targets: SendTargets::All,
                message: BallCollision {
                    ball,
                    target,
                    position: impact.point.as_vec3(),
                    speed: -impact.normal_speed as f32,
                },
            });
        }
    }
}

fn send_sensor_entries(
    trigger: On<CollisionStart>,
    balls: Query<(&Position, &LinearVelocity), With<Player>>,
    sensors: Query<(), With<Sensor>>,
    targets: CollisionTargets,
    mut writer: MessageWriter<ToClients<BallCollision>>,
) {
    // Triggered for both entities if both have collision events enabled, only handle the one
    // for the ball.
    let ball = trigger.collider1;
    let Ok((position, velocity)) = balls.get(ball) else {
        return;
    };

    let other = trigger.collider2;
    if !sensors.contains(other) {
        return;
    }

    let Some(target) = targets.get(other) else {
        return;
    };

    writer.write(ToClients {
        targets: SendTargets::All,
        message: BallCollision {
            ball,
            target,
            position: position.0.as_vec3(),
            speed: velocity.0.length() as f32,
        },
    });
}
