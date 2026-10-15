//! A spring and damper per wheel, acting along the body's up axis.

/// Force the suspension pushes the body up with, in newtons.
///
/// `compression` is how far the spring is shortened from rest, metres.
/// `closing_speed` is how fast it is shortening, m/s. Both positive when the
/// wheel is being pushed into the body. The result never pulls the body
/// down: a spring that is extending faster than it is loaded goes slack.
pub fn force(stiffness: f32, damping: f32, compression: f32, closing_speed: f32) -> f32 {
    (stiffness * compression + damping * closing_speed).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::force;

    #[test]
    fn static_compression_is_hookes_law() {
        assert_eq!(force(1000.0, 50.0, 0.1, 0.0), 100.0);
    }

    #[test]
    fn closing_adds_damping() {
        assert_eq!(force(1000.0, 50.0, 0.1, 2.0), 200.0);
    }

    #[test]
    fn never_pulls_down() {
        assert_eq!(force(1000.0, 50.0, 0.01, -10.0), 0.0);
    }
}
