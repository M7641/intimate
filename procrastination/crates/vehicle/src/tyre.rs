//! Contact-patch forces. A deliberately simple model: lateral force grows
//! with sideslip speed and saturates, longitudinal force is whatever the
//! drivetrain and brakes ask for, and the two are clipped to a friction
//! circle so the tyre never exceeds `grip * load` in total.

use glam::Vec2;

use crate::GRAVITY;

/// What the caller knows about one wheel's contact this step.
#[derive(Debug, Clone, Copy)]
pub struct Contact {
    /// Vertical load on the tyre, newtons.
    pub load: f32,
    /// Peak friction coefficient.
    pub grip: f32,
    /// Lateral stiffness, N per (m/s) per N of load.
    pub lateral_stiffness: f32,
    /// Contact-patch velocity along the wheel's rolling direction, m/s.
    pub forward_speed: f32,
    /// Contact-patch velocity across the wheel, m/s. Positive to the right.
    pub side_speed: f32,
}

/// Longitudinal and lateral tyre force in the wheel's own frame: `x` along
/// the rolling direction, `y` to the right.
///
/// `drive` is the force the drivetrain wants to push with (negative in
/// reverse). `brake` is the braking force magnitude, always resisting
/// motion. `dt` lets braking stop the wheel dead instead of reversing it.
pub fn force(contact: Contact, drive: f32, brake: f32, dt: f32) -> Vec2 {
    if contact.load <= 0.0 {
        return Vec2::ZERO;
    }
    let longitudinal = drive + brake_force(contact, brake, dt);
    let lateral = -contact.side_speed * contact.lateral_stiffness * contact.load;
    clip_to_circle(
        Vec2::new(longitudinal, lateral),
        contact.grip * contact.load,
    )
}

/// Braking opposes the forward speed, but is capped at the force that would
/// bring this corner's share of the mass to rest within one step. Without
/// the cap the sign flips every step at low speed and the car jitters.
fn brake_force(contact: Contact, brake: f32, dt: f32) -> f32 {
    if brake <= 0.0 || dt <= 0.0 {
        return 0.0;
    }
    let supported_mass = contact.load / GRAVITY;
    let to_rest = contact.forward_speed.abs() * supported_mass / dt;
    -contact.forward_speed.signum() * brake.min(to_rest)
}

fn clip_to_circle(force: Vec2, limit: f32) -> Vec2 {
    let magnitude = force.length();
    if magnitude > limit {
        force * (limit / magnitude)
    } else {
        force
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn contact(forward_speed: f32, side_speed: f32) -> Contact {
        Contact {
            load: 3000.0,
            grip: 1.0,
            lateral_stiffness: 4.0,
            forward_speed,
            side_speed,
        }
    }

    #[test]
    fn no_load_no_force() {
        let c = Contact {
            load: 0.0,
            ..contact(5.0, 1.0)
        };
        assert_eq!(force(c, 1000.0, 500.0, 0.016), Vec2::ZERO);
    }

    #[test]
    fn sideslip_is_resisted() {
        let f = force(contact(10.0, 0.01), 0.0, 0.0, 0.016);
        assert!(f.y < 0.0);
    }

    #[test]
    fn braking_opposes_motion_and_does_not_reverse() {
        let slow = contact(0.001, 0.0);
        let f = force(slow, 0.0, 5000.0, 0.016);
        assert!(f.x < 0.0);
        let stopping = slow.load / GRAVITY * slow.forward_speed / 0.016;
        assert!((f.x + stopping).abs() < 1e-3);
    }

    proptest! {
        #[test]
        fn never_exceeds_friction_circle(
            forward in -50.0f32..50.0,
            side in -20.0f32..20.0,
            drive in -10_000.0f32..10_000.0,
            brake in 0.0f32..20_000.0,
        ) {
            let c = contact(forward, side);
            let f = force(c, drive, brake, 1.0 / 60.0);
            prop_assert!(f.length() <= c.grip * c.load * 1.0001);
        }
    }
}
