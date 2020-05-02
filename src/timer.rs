use std::time::Duration;


/// An interval descibes of a duration of time passed between successive updates 
/// of a timer.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Interval {
    Milliseconds(u64),
}

/// A timer is a type that tracks time using an event counter.
pub struct Timer {
    /// The time elapsed since the last reset of the timer.
    time: Duration,
    /// The period between events tracked by the timer.
    event_interval: Duration,
    /// The number of events triggered since the last reset of the timer.
    event_count: u128,
}

impl Timer {
    /// Construct a new timer.
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

    /// Update the state of the timer.
    #[inline]
    pub fn update(&mut self, elapsed: Duration) {
        self.time += elapsed;
        self.event_count = self.time.as_millis() / self.event_interval.as_millis();
    }

    /// Determine whether an event has triggered since the timer was reset.
    #[inline]
    pub fn event_triggered(&self) -> bool {
        self.event_count > 0
    }

    /// Reset the timer state.
    #[inline]
    pub fn reset(&mut self) {
        self.time = Duration::from_millis(0);
        self.event_count = 0;
    }
}
