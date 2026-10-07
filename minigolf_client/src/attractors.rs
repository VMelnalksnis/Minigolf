use {
    bevy::prelude::*,
    minigolf::Attractor,
    std::f32::consts::{FRAC_PI_2, PI},
};

/// Shows the range of [Attractor]s, e.g. black hole bumpers and the hole magnet.
pub(crate) struct AttractorEffectPlugin;

impl Plugin for AttractorEffectPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<AttractorRing>();

        app.add_observer(on_attractor_added);
        app.add_systems(Update, animate_attractor_rings);
    }
}

const COLOR: Color = Color::srgb(0.55, 0.3, 0.95);
const RANGE_ALPHA: f32 = 0.15;
const RING_ALPHA: f32 = 0.6;
/// Width of the rings, relative to the radius of the attractor.
const RING_WIDTH: f32 = 0.06;
const RING_COUNT: usize = 3;
/// Time (s) it takes for a ring to move from the edge of the range to the center.
const RING_PERIOD: f32 = 1.5;
/// Height above the attractor, so that the effect is not hidden by the floor.
const HEIGHT: f32 = 0.006;

/// A ring that moves from the edge of the range of the parent [Attractor] towards its center.
#[derive(Component, Reflect, Debug)]
struct AttractorRing {
    /// Offset of the animation, so that the rings are spread out.
    offset: f32,
}

/// Adds a disc showing the range of the attractor, with rings moving towards its center.
fn on_attractor_added(
    add: On<Add, Attractor>,
    attractors: Query<&Attractor>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let attractor = attractors.get(add.entity).unwrap();

    // Meshes are in the XY plane, rotate them to lie on the floor.
    let flat = |height: f32| {
        Transform::from_xyz(0.0, height, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
    };

    let ring_mesh = meshes.add(Annulus::new(1.0 - RING_WIDTH, 1.0));

    commands
        .entity(add.entity)
        .insert(Visibility::default())
        .with_children(|parent| {
            parent.spawn((
                Name::new("Attractor range"),
                Mesh3d(meshes.add(Circle::new(attractor.radius))),
                MeshMaterial3d(materials.add(material(RANGE_ALPHA))),
                flat(HEIGHT),
            ));

            for index in 0..RING_COUNT {
                parent.spawn((
                    Name::new("Attractor ring"),
                    AttractorRing {
                        offset: index as f32 / RING_COUNT as f32,
                    },
                    Mesh3d(ring_mesh.clone()),
                    // Each ring fades separately, so it needs its own material.
                    MeshMaterial3d(materials.add(material(0.0))),
                    // Above the range, so that they are not sorted behind it.
                    flat(HEIGHT * 1.5),
                ));
            }
        });
}

fn material(alpha: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: COLOR.with_alpha(alpha),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

fn animate_attractor_rings(
    time: Res<Time>,
    attractors: Query<&Attractor>,
    mut rings: Query<(
        &AttractorRing,
        &ChildOf,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (ring, child_of, mut transform, material) in &mut rings {
        let Ok(attractor) = attractors.get(child_of.parent()) else {
            continue;
        };

        let progress = (time.elapsed_secs() / RING_PERIOD + ring.offset).fract();
        let radius = attractor.radius + (attractor.min_radius - attractor.radius) * progress;
        transform.scale = Vec3::splat(radius.max(0.001));

        // Fade in at the edge of the range and out towards the center.
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.base_color = COLOR.with_alpha(RING_ALPHA * (progress * PI).sin());
        }
    }
}
