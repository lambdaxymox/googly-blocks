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



/// A specification type describing the parameters for constructing a 
/// flashing animation state machine.
#[derive(Copy, Clone)]
pub struct FlashAnimationStateMachineSpec {
    pub flash_switch_interval: Interval,
    pub flash_stop_interval: Interval,
}

/// A factory method for creating a flash animation state machine from a 
/// specification.
pub fn create(spec: FlashAnimationStateMachineSpec) -> FlashAnimationStateMachine {
    FlashAnimationStateMachine::new(spec.flash_switch_interval, spec.flash_stop_interval)
}

/// The internal state of the animation state machine.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FlashAnimationState {
    /// Display the light color.
    Light,
    /// Display the dark color.
    Dark,
    /// Do not run the animation.
    Disabled,
}

/// The flash animation state machine that coordinates the execution of the 
/// background flash animation when the player gets a tetris during gameplay.
pub struct FlashAnimationStateMachine {
    /// The current state of the animation.
    pub state: FlashAnimationState,
    /// The timer that controls the duration for switching between animation states.
    flash_switch_timer: Timer,
    /// The timer that controls the duration of the entire background flash animation.
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

    /// Determine whether the flash animation is enabled.
    #[inline]
    fn is_enabled(&self) -> bool {
        self.state != FlashAnimationState::Disabled
    }

    /// Enable the flash animation.
    #[inline]
    pub fn enable(&mut self) {
        self.state = FlashAnimationState::Dark;
        self.flash_stop_timer.reset();
        self.flash_switch_timer.reset();
    }

    /// Disable the flash animation.
    #[inline]
    pub fn disable(&mut self) {
        self.state = FlashAnimationState::Disabled;
        self.flash_stop_timer.reset();
        self.flash_switch_timer.reset();
    }

    /// Determine whether the flash animation is disabled.
    #[inline]
    pub fn is_disabled(&self) -> bool {
        self.state == FlashAnimationState::Disabled
    }

    /// Update the internal state of the state machine.
    #[inline]
    fn update_state(&mut self) {
        self.state = match self.state {
            FlashAnimationState::Disabled => FlashAnimationState::Disabled,
            FlashAnimationState::Dark => FlashAnimationState::Light,
            FlashAnimationState::Light => FlashAnimationState::Dark,
        };
    }

    /// Update the state machine with the change in the time elapsed in between
    /// state updates.
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


#[cfg(test)]
mod tests {
    use super::{
        FlashAnimationState,
        FlashAnimationStateMachine,
    };
    use timer::Interval;
    use std::time::Duration;

    /// The flash animation state machine should correctly enable itself after calling the enable function.
    #[test]
    fn state_machine_should_correctly_report_being_enabled_after_enabling() {
        let flash_switch_interval = Interval::Milliseconds(50);
        let flash_stop_interval = Interval::Milliseconds(1000);
        let mut state_machine = FlashAnimationStateMachine::new(flash_switch_interval, flash_stop_interval);
        state_machine.enable();

        assert!(state_machine.is_enabled());
    }

    /// Calling disable on the flash animation state machine should disable the flashing animation.
    #[test]
    fn state_machine_should_correctly_report_being_disabled_after_disabling() {
        let flash_switch_interval = Interval::Milliseconds(50);
        let flash_stop_interval = Interval::Milliseconds(1000);
        let mut state_machine = FlashAnimationStateMachine::new(flash_switch_interval, flash_stop_interval);
        state_machine.enable();
        state_machine.disable();

        assert!(state_machine.is_disabled());
    }

    /// The flashing animation should proceed up to the stopping time of the animations. Afterwards,
    /// it should be disabled.
    #[test]
    fn state_machine_should_disable_after_stopping_time_is_reached() {
        let flash_switch_interval = Interval::Milliseconds(50);
        let flash_stop_interval = Interval::Milliseconds(1000);
        let mut state_machine = FlashAnimationStateMachine::new(flash_switch_interval, flash_stop_interval);
        let elapsed_milliseconds = Duration::from_millis(1001);
        state_machine.enable();
        state_machine.update(elapsed_milliseconds);

        assert!(state_machine.is_disabled());
    }

    /// When the elapsed time during the flashing animation is less than the stopping time, the animation
    /// should remain enabled unless someone explicitly disables it beforehand.
    #[test]
    fn state_machine_should_remain_enabled_before_stopping_time_is_reached() {
        let flash_switch_interval = Interval::Milliseconds(50);
        let flash_stop_interval = Interval::Milliseconds(1000);
        let mut state_machine = FlashAnimationStateMachine::new(flash_switch_interval, flash_stop_interval);
        let elapsed_milliseconds = Duration::from_millis(999);
        state_machine.enable();
        state_machine.update(elapsed_milliseconds);

        assert!(state_machine.is_enabled());
    }

    /// The flashing animation should correctly switch between frames of the animation 
    /// after each switching period.
    #[test]
    fn state_machine_should_transition_from_dark_to_light_after_switch_time_is_reached() {
        let flash_switch_interval = Interval::Milliseconds(50);
        let flash_stop_interval = Interval::Milliseconds(1000);
        let mut state_machine = FlashAnimationStateMachine::new(flash_switch_interval, flash_stop_interval);
        let elapsed_milliseconds = Duration::from_millis(50);
        state_machine.enable();
        state_machine.update(elapsed_milliseconds);

        assert_eq!(state_machine.state, FlashAnimationState::Light);
    }

    /// The flashing animation should correctly switch between frames of the animation 
    /// after each switching period.
    #[test]
    fn state_machine_should_transition_from_light_to_dark_after_switch_time_is_reached() {
        let flash_switch_interval = Interval::Milliseconds(50);
        let flash_stop_interval = Interval::Milliseconds(1000);
        let mut state_machine = FlashAnimationStateMachine::new(flash_switch_interval, flash_stop_interval);
        let elapsed_milliseconds = Duration::from_millis(50);
        state_machine.enable();
        state_machine.update(elapsed_milliseconds);
        state_machine.update(elapsed_milliseconds);

        assert_eq!(state_machine.state, FlashAnimationState::Dark);
    }

    /// Subsequent updates to the flash animation state machine should not disable the animation until
    /// the total elapsed time has passed the stopping time.
    #[test]
    fn state_machine_should_remain_enabled_before_stopping_time() {
        let flash_switch_interval = Interval::Milliseconds(50);
        let flash_stop_interval = Interval::Milliseconds(1000);
        let mut state_machine = FlashAnimationStateMachine::new(flash_switch_interval, flash_stop_interval);
        let elapsed_milliseconds = Duration::from_millis(50);
        state_machine.enable();
        state_machine.update(elapsed_milliseconds);
        state_machine.update(elapsed_milliseconds);

        assert!(state_machine.is_enabled());
    }
}
