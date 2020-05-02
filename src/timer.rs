use std::time::Duration;


#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Interval {
    Milliseconds(u64),
}

pub struct Timer {
    time: Duration,
    event_interval: Duration,
    event_count: u128,
}

impl Timer {
    pub fn new(interval: Interval) -> Timer {
        let event_interval = match interval {
            Interval::Milliseconds(millis) => Duration::from_millis(millis)
        };
        
        Timer {
            time: Duration::from_millis(0),
            event_interval: event_interval,
            event_count: 0,
        }
    }

    #[inline]
    pub fn update(&mut self, elapsed: Duration) {
        self.time += elapsed;
        self.event_count = self.time.as_millis() / self.event_interval.as_millis();
    }

    #[inline]
    pub fn event_triggered(&self) -> bool {
        self.event_count > 0
    }

    #[inline]
    pub fn reset(&mut self) {
        self.time = Duration::from_millis(0);
        self.event_count = 0;
    }
}
