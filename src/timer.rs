/*
 *  Googly Blocks is a video game.
 *  Copyright (C) 2018,2019,2020  Christopher Blanchard
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */
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


#[cfg(test)]
mod tests {
    use super::{
        Interval, 
        Timer
    };
    use std::time::Duration;


    #[test]
    fn timer_correctly_triggers_an_event_after_event_duration() {
        let mut timer = Timer::new(Interval::Milliseconds(100));
        let elapsed_milliseconds = Duration::from_millis(101);
        timer.update(elapsed_milliseconds);

        assert!(timer.event_triggered());
    }

    #[test]
    fn timer_correctly_counts_events_between_resets() {
        let mut timer = Timer::new(Interval::Milliseconds(100));
        let elapsed_milliseconds = Duration::from_millis(1000);
        timer.reset();
        timer.update(elapsed_milliseconds);

        assert_eq!(timer.event_count, 10);
    }
}
