use crate::server::ValidPlayerInput;
use bevy::app::App;
use bevy::math::Vec3;
use bevy::prelude::*;

pub(crate) struct CoursePlugin;

impl Plugin for CoursePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Course>()
            .register_type::<Hole>()
            .register_type::<PlayerScore>();

        app.add_observer(on_hole_added);

        app.add_systems(Update, (increment_score, log_score_changes));
    }
}

#[derive(Component, Reflect, Debug)]
pub(crate) struct Course {
    holes: Vec<Entity>,
}

impl Course {
    pub(crate) fn new() -> Self {
        Course { holes: vec![] }
    }
}

#[derive(Component, Reflect, Debug)]
pub(crate) struct Hole {
    start_position: Vec3,
}
impl Hole {
    pub(crate) fn new() -> Self {
        Hole {
            start_position: Vec3::ZERO,
        }
    }
}

#[derive(Component, Reflect, Default, Debug)]
pub(crate) struct PlayerScore {
    score: u32,
}

fn on_hole_added(trigger: Trigger<OnAdd, Hole>, mut course: Query<&mut Course>) {
    let hole_entity = trigger.entity();
    let mut course = course.single_mut();
    course.holes.push(hole_entity);
}

fn increment_score(mut reader: EventReader<ValidPlayerInput>, mut scores: Query<&mut PlayerScore>) {
    for input in reader.read() {
        let Ok(mut score) = scores.get_mut(input.player) else {
            warn!("Received {:?} without player score component", input);
            continue;
        };

        score.score += 1;
    }
}

fn log_score_changes(scores: Query<(Entity, &PlayerScore), Changed<PlayerScore>>) {
    for (entity, score) in scores.iter() {
        info!(
            "Increased score to {:?} for player {:?}",
            score.score, entity
        );
    }
}
