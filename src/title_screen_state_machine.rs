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
use crate::timer::{
    Timer,
    Interval,
};
use std::time::{
    Duration
};


 #[derive(Clone)]
pub struct TitleScreenStateMachineSpec {
    pub transition_interval: Interval,
    pub pressed_interval: Interval,
    pub unpressed_interval: Interval,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum TitleScreenBlinkState {
    Disabled,
    Unpressed,
    Pressed,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum TitleScreenAnimationState {
    Disabled,
    On,
    Off,
}

pub struct TitleScreenBlinkStateMachine {
    state: TitleScreenBlinkState,
    animation_state: TitleScreenAnimationState,
    unpressed_blink_timer: Timer,
    pressed_blink_timer: Timer,
}

impl TitleScreenBlinkStateMachine {
    fn new(spec: &TitleScreenStateMachineSpec) -> TitleScreenBlinkStateMachine {
        TitleScreenBlinkStateMachine {
            state: TitleScreenBlinkState::Disabled,
            animation_state: TitleScreenAnimationState::Disabled,
            unpressed_blink_timer: Timer::new(spec.unpressed_interval),
            pressed_blink_timer: Timer::new(spec.pressed_interval),
        }
    }

    #[inline]
    fn is_enabled(&self) -> bool {
        self.state != TitleScreenBlinkState::Disabled
    }

    #[inline]
    pub fn enable(&mut self) {
        if self.state == TitleScreenBlinkState::Disabled {
            self.state = TitleScreenBlinkState::Unpressed;
            self.animation_state = TitleScreenAnimationState::On;
            self.unpressed_blink_timer.reset();
            self.pressed_blink_timer.reset();
        }
    }

    #[inline]
    pub fn disable(&mut self) {
        self.state = TitleScreenBlinkState::Disabled;
        self.animation_state = TitleScreenAnimationState::Disabled;
        self.unpressed_blink_timer.reset();
        self.pressed_blink_timer.reset();
    }

    #[inline]
    pub fn is_disabled(&self) -> bool {
        self.state == TitleScreenBlinkState::Disabled
    }

    #[inline]
    fn unpressed(&mut self) {
        self.state = TitleScreenBlinkState::Unpressed;
        self.animation_state = TitleScreenAnimationState::On;
        self.unpressed_blink_timer.reset();
        self.pressed_blink_timer.reset();
    }

    #[inline]
    fn is_unpressed(&self) -> bool {
        self.state == TitleScreenBlinkState::Unpressed
    }

    #[inline]
    pub fn pressed(&mut self) {
        self.state = TitleScreenBlinkState::Pressed;
        self.animation_state = TitleScreenAnimationState::On;
        self.unpressed_blink_timer.reset();
        self.pressed_blink_timer.reset();
    }

    #[inline]
    pub fn is_pressed(&self) -> bool {
        self.state == TitleScreenBlinkState::Pressed
    }

    #[inline]
    fn animation_is_on(&self) -> bool {
        self.animation_state == TitleScreenAnimationState::On
    }

    #[inline]
    pub fn update(&mut self, elapsed_milliseconds: Duration) {
        match self.state {
            TitleScreenBlinkState::Disabled => {}
            TitleScreenBlinkState::Unpressed => {
                self.pressed_blink_timer.reset();
                self.unpressed_blink_timer.update(elapsed_milliseconds);
                if self.unpressed_blink_timer.event_triggered() {
                    self.animation_state = match self.animation_state {
                        TitleScreenAnimationState::On => TitleScreenAnimationState::Off,
                        TitleScreenAnimationState::Off => TitleScreenAnimationState::On,
                        TitleScreenAnimationState::Disabled => TitleScreenAnimationState::Disabled,
                    };
                    self.unpressed_blink_timer.reset();
                }
            }
            TitleScreenBlinkState::Pressed => {
                self.unpressed_blink_timer.reset();
                self.pressed_blink_timer.update(elapsed_milliseconds);
                if self.pressed_blink_timer.event_triggered() {
                    self.animation_state = match self.animation_state {
                        TitleScreenAnimationState::On => TitleScreenAnimationState::Off,
                        TitleScreenAnimationState::Off => TitleScreenAnimationState::On,
                        TitleScreenAnimationState::Disabled => TitleScreenAnimationState::Disabled,
                    };
                    self.pressed_blink_timer.reset();
                }
            }
        }
    }
}

pub struct TitleScreenStateMachine {
    pub blink_state: TitleScreenBlinkStateMachine,
    pub transition_timer: Timer,
}

impl TitleScreenStateMachine {
    pub fn new(spec: TitleScreenStateMachineSpec) -> TitleScreenStateMachine {
        let mut blink_state = TitleScreenBlinkStateMachine::new(&spec);
        blink_state.unpressed(); 
        TitleScreenStateMachine {
            blink_state: blink_state,
            transition_timer: Timer::new(spec.transition_interval),
        }
    }
    
    #[inline]
    pub fn animation_is_on(&self) -> bool {
        self.blink_state.animation_is_on()
    }
}
