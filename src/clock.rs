/// A discrete simulation tick. All time in the simulator is measured in ticks.
pub type Tick = u64;

/// Simulation clock that tracks the current tick.
///
/// The clock only advances when the engine explicitly advances it — there is no
/// wall-clock coupling.
#[derive(Debug, Clone)]
pub struct Clock {
    current: Tick,
}

impl Clock {
    pub fn new() -> Self {
        Self { current: 0 }
    }

    pub fn now(&self) -> Tick {
        self.current
    }

    pub fn advance_to(&mut self, tick: Tick) {
        debug_assert!(tick >= self.current, "clock must not move backwards");
        self.current = tick;
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_starts_at_zero() {
        let clock = Clock::new();
        assert_eq!(clock.now(), 0);
    }

    #[test]
    fn clock_advances() {
        let mut clock = Clock::new();
        clock.advance_to(42);
        assert_eq!(clock.now(), 42);
    }
}
