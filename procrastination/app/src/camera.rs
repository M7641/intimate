//! A chase camera that trails the player's car.

use bevy::prelude::*;

use crate::scene::Player;

#[derive(Component)]
pub struct ChaseCamera {
    /// Offset from the car in the car's frame: up and behind.
    pub offset: Vec3,
    /// How quickly the camera catches up, per second.
    pub stiffness: f32,
}

impl Default for ChaseCamera {
    fn default() -> Self {
        Self {
            offset: Vec3::new(0.0, 3.0, 8.0),
            stiffness: 4.0,
        }
    }
}

pub fn follow(
    time: Res<Time>,
    player: Query<&GlobalTransform, With<Player>>,
    mut cameras: Query<(&ChaseCamera, &mut Transform), Without<Player>>,
) {
    let Ok(target) = player.single() else { return };
    let (_, rotation, translation) = target.to_scale_rotation_translation();
    // Follow the heading only, not roll or pitch, so bumps do not toss the view.
    let heading = Quat::from_rotation_y(rotation.to_euler(EulerRot::YXZ).0);
    for (chase, mut transform) in &mut cameras {
        let desired = translation + heading * chase.offset;
        let blend = 1.0 - (-chase.stiffness * time.delta_secs()).exp();
        transform.translation = transform.translation.lerp(desired, blend);
        transform.look_at(translation + Vec3::Y * 0.8, Vec3::Y);
    }
}
