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
use timer::{
    Interval,
    Timer,
};
use std::time::Duration;


#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FlashAnimationState {
    Light,
    Dark,
    Disabled,
}

pub struct FlashAnimationStateMachine {
    pub state: FlashAnimationState,
    flash_switch_timer: Timer,
    flash_stop_timer: Timer,
}

impl FlashAnimationStateMachine {
    pub fn new(flash_switch_interval: Interval, flash_stop_interval: Interval) -> FlashAnimationStateMachine {
        FlashAnimationStateMachine {
            state: FlashAnimationState::Disabled,
            flash_switch_timer: Timer::new(flash_switch_interval),
            flash_stop_timer: Timer::new(flash_stop_interval),
        }
    }

    #[inline]
    fn is_enabled(&self) -> bool {
        self.state != FlashAnimationState::Disabled
    }

    #[inline]
    pub fn enable(&mut self) {
        self.state = FlashAnimationState::Dark;
    }

    #[inline]
    pub fn disable(&mut self) {
        self.state = FlashAnimationState::Disabled;
    }

    #[inline]
    pub fn is_disabled(&self) -> bool {
        self.state == FlashAnimationState::Disabled
    }

    #[inline]
    fn update_state(&mut self) {
        self.state = match self.state {
            FlashAnimationState::Disabled => FlashAnimationState::Disabled,
            FlashAnimationState::Dark => FlashAnimationState::Light,
            FlashAnimationState::Light => FlashAnimationState::Dark,
        };
    }

    pub fn update(&mut self, elapsed_milliseconds: Duration) {
        if self.is_enabled() {
            self.flash_switch_timer.update(elapsed_milliseconds);
            self.flash_stop_timer.update(elapsed_milliseconds);
            if self.flash_stop_timer.event_triggered() {
                self.flash_switch_timer.reset();
                self.flash_stop_timer.reset();
                self.disable();
            } else if self.flash_switch_timer.event_triggered() {
                self.flash_switch_timer.reset();
                self.update_state();
            }
        }
    }
}
