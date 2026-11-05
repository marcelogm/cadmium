use std::time::Instant;

const MILLISECONDS_PER_SECOND: f64 = 1000.0;

pub trait Clock {
    fn now_ms(&self) -> f64;
}

pub struct SystemClock {
    started: Instant,
}

impl SystemClock {
    pub fn new() -> SystemClock {
        SystemClock {
            started: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> SystemClock {
        SystemClock::new()
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * MILLISECONDS_PER_SECOND
    }
}
