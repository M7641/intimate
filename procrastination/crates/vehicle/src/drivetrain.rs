//! Engine, clutch and gearbox. The engine is a flywheel driven by a torque
//! curve; the clutch is a torque-limited coupling to the wheels; the gearbox
//! is a ratio. Letting the clutch out too fast at low revs stalls the
//! engine, which is most of what a learner needs to feel.

use crate::{Controls, VehicleSpec};

const RPM_PER_RAD_S: f32 = 60.0 / (2.0 * std::f32::consts::PI);

/// Mutable drivetrain state. Create one per vehicle and step it each tick.
#[derive(Debug, Clone, PartialEq)]
pub struct Drivetrain {
    /// Engine angular speed, rad/s.
    pub engine_speed: f32,
    pub running: bool,
}

impl Default for Drivetrain {
    fn default() -> Self {
        Self {
            engine_speed: 0.0,
            running: false,
        }
    }
}

impl Drivetrain {
    /// Turn the key. Spins the engine up to idle.
    pub fn start(&mut self, spec: &VehicleSpec) {
        self.running = true;
        self.engine_speed = spec.idle_rpm / RPM_PER_RAD_S;
    }

    pub fn rpm(&self) -> f32 {
        self.engine_speed * RPM_PER_RAD_S
    }

    /// Advance one step and return the torque delivered to the driven axle,
    /// N·m, positive pushing the car forward.
    ///
    /// `wheel_speed` is the driven wheels' mean angular speed, rad/s,
    /// positive rolling forward.
    pub fn step(
        &mut self,
        spec: &VehicleSpec,
        controls: &Controls,
        wheel_speed: f32,
        dt: f32,
    ) -> f32 {
        let engine_torque = self.engine_torque(spec, controls.throttle);
        let (clutch_torque, axle_torque) = match spec.ratio(controls.gear) {
            Some(ratio) => {
                let engagement = (1.0 - controls.clutch).clamp(0.0, 1.0);
                let capacity = spec.clutch_torque * engagement;
                let slip = self.engine_speed - wheel_speed * ratio;
                // The torque that would kill the slip within this step, clipped to
                // what the plates can hold. Solving for the step keeps it stable
                // at any `dt`; a fixed stiffness would chatter.
                let to_lock = slip * spec.engine_inertia / dt.max(1e-4);
                let torque = to_lock.clamp(-capacity, capacity);
                (torque, torque * ratio)
            }
            None => (0.0, 0.0),
        };
        let acceleration = (engine_torque - clutch_torque) / spec.engine_inertia;
        self.engine_speed = (self.engine_speed + acceleration * dt).max(0.0);
        self.check_stall(spec);
        axle_torque
    }

    /// Net torque from combustion minus internal friction. A flat curve that
    /// falls away past the redline is enough for now.
    fn engine_torque(&self, spec: &VehicleSpec, throttle: f32) -> f32 {
        if !self.running {
            return 0.0;
        }
        let rpm = self.rpm();
        let idle_assist = ((spec.idle_rpm - rpm) / spec.idle_rpm).clamp(0.0, 1.0);
        let demand = throttle.clamp(0.0, 1.0).max(idle_assist);
        let over_redline = ((rpm - spec.redline_rpm) / 500.0).clamp(0.0, 1.0);
        let combustion = spec.engine_torque * demand * (1.0 - over_redline);
        let friction = spec.engine_torque * 0.15 * (rpm / spec.redline_rpm);
        combustion - friction
    }

    fn check_stall(&mut self, spec: &VehicleSpec) {
        if self.running && self.rpm() < spec.stall_rpm {
            self.running = false;
            self.engine_speed = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(drivetrain: &mut Drivetrain, spec: &VehicleSpec, controls: &Controls, steps: usize) {
        for _ in 0..steps {
            drivetrain.step(spec, controls, 0.0, 1.0 / 60.0);
        }
    }

    #[test]
    fn holds_idle_in_neutral() {
        let spec = VehicleSpec::sedan();
        let mut d = Drivetrain::default();
        d.start(&spec);
        settle(&mut d, &spec, &Controls::default(), 600);
        assert!(d.running);
        assert!((d.rpm() - spec.idle_rpm).abs() < 100.0, "rpm {}", d.rpm());
    }

    #[test]
    fn neutral_sends_no_torque() {
        let spec = VehicleSpec::sedan();
        let mut d = Drivetrain::default();
        d.start(&spec);
        let controls = Controls {
            throttle: 1.0,
            ..Default::default()
        };
        assert_eq!(d.step(&spec, &controls, 0.0, 1.0 / 60.0), 0.0);
    }

    #[test]
    fn dumping_the_clutch_at_idle_stalls() {
        let spec = VehicleSpec::sedan();
        let mut d = Drivetrain::default();
        d.start(&spec);
        let controls = Controls {
            gear: 1,
            clutch: 0.0,
            ..Default::default()
        };
        // Wheels held still: the clutch drags the engine down.
        settle(&mut d, &spec, &controls, 120);
        assert!(!d.running);
    }

    #[test]
    fn clutch_pressed_keeps_engine_alive_in_gear() {
        let spec = VehicleSpec::sedan();
        let mut d = Drivetrain::default();
        d.start(&spec);
        let controls = Controls {
            gear: 1,
            clutch: 1.0,
            ..Default::default()
        };
        settle(&mut d, &spec, &controls, 120);
        assert!(d.running);
    }

    #[test]
    fn reverse_ratio_is_negative() {
        let spec = VehicleSpec::sedan();
        assert!(spec.ratio(-1).unwrap() < 0.0);
        assert!(spec.ratio(1).unwrap() > 0.0);
        assert!(spec.ratio(0).is_none());
        assert!(spec.ratio(9).is_none());
    }
}
