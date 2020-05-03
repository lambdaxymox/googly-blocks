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
use input::{
    InputKind,
    InputAction,
    Input,
};
use flash_state_machine::{
    FlashAnimationStateMachine,
};
use next_block::NextBlockCell;
use playing_field::{
    GooglyBlockMove,
    PlayingFieldContext,
};
use score::{
    ScoreBoard,
    Statistics,
};
use timer::{
    Interval,
    Timer,
};

use std::rc::Rc;
use std::cell::RefCell;
use std::time::Duration;


pub fn create(spec: PlayingFieldStateMachineSpec) -> PlayingFieldStateMachine {
    let timers = Rc::new(RefCell::new(PlayingFieldTimers::new(spec.timers)));
    let full_rows = Rc::new(RefCell::new(FullRows::new()));
    let flashing_state_machine = Rc::new(RefCell::new(
        FlashAnimationStateMachine::new(spec.flash_timers.flash_switch_interval, spec.flash_timers.flash_stop_interval)
    ));
    let context = Rc::new(RefCell::new(PlayingFieldStateMachineContext {
        timers: timers,
        playing_field_state: spec.playing_field_context,
        next_block: spec.next_block,
        statistics: spec.statistics,
        score_board: spec.score_board,
        full_rows: full_rows,
        flashing_state_machine: flashing_state_machine,
        columns_cleared: 0,
    }));

    PlayingFieldStateMachine::new(context)
}


#[derive(Copy, Clone)]
pub struct PlayingFieldTimerSpec {
    pub fall_interval: Interval,
    pub collision_interval: Interval,
    pub left_hold_interval: Interval,
    pub right_hold_interval: Interval,
    pub down_hold_interval: Interval,
    pub rotate_interval: Interval,
    pub clearing_interval: Interval,
}

#[derive(Copy, Clone)]
pub struct FlashAnimationStateMachineSpec {
    pub flash_switch_interval: Interval,
    pub flash_stop_interval: Interval,
}

#[derive(Clone)]
pub struct PlayingFieldStateMachineSpec {
    pub timers: PlayingFieldTimerSpec,
    pub flash_timers: FlashAnimationStateMachineSpec,
    pub playing_field_context: Rc<RefCell<PlayingFieldContext>>,
    pub next_block: Rc<RefCell<NextBlockCell>>,
    pub statistics: Rc<RefCell<Statistics>>,
    pub score_board: Rc<RefCell<ScoreBoard>>,
}

struct PlayingFieldTimers {
    fall_timer: Timer,
    collision_timer: Timer,
    left_hold_timer: Timer,
    right_hold_timer: Timer,
    down_hold_timer: Timer,
    rotate_timer: Timer,
    clearing_timer: Timer,
}

impl PlayingFieldTimers {
    fn new(spec: PlayingFieldTimerSpec) -> PlayingFieldTimers {
        PlayingFieldTimers {
            fall_timer: Timer::new(spec.fall_interval),
            collision_timer: Timer::new(spec.collision_interval),
            left_hold_timer: Timer::new(spec.left_hold_interval),
            right_hold_timer: Timer::new(spec.right_hold_interval),
            down_hold_timer: Timer::new(spec.down_hold_interval),
            rotate_timer: Timer::new(spec.rotate_interval),
            clearing_timer: Timer::new(spec.clearing_interval),
        }
    }
}

struct FullRows {
    rows: [isize; 20],
    count: usize,
}

impl FullRows {
    fn new() -> FullRows {
        FullRows {
            rows: [-1; 20],
            count: 0,
        }
    }

    fn clear(&mut self) {
        for i in 0..self.rows.len() {
            self.rows[i] = -1;
        }
        self.count = 0;
    }
}
/*
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
    fn new(flash_switch_interval: Interval, flash_stop_interval: Interval) -> FlashAnimationStateMachine {
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
    fn enable(&mut self) {
        self.state = FlashAnimationState::Dark;
    }

    #[inline]
    pub fn disable(&mut self) {
        self.state = FlashAnimationState::Disabled;
    }

    #[inline]
    fn is_disabled(&self) -> bool {
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

    fn update(&mut self, elapsed_milliseconds: Duration) {
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
*/

struct PlayingFieldStateMachineContext {
    timers: Rc<RefCell<PlayingFieldTimers>>,
    playing_field_state: Rc<RefCell<PlayingFieldContext>>,
    next_block: Rc<RefCell<NextBlockCell>>,
    statistics: Rc<RefCell<Statistics>>,
    score_board: Rc<RefCell<ScoreBoard>>,
    full_rows: Rc<RefCell<FullRows>>,
    flashing_state_machine: Rc<RefCell<FlashAnimationStateMachine>>,
    columns_cleared: usize,
}

#[derive(Copy, Clone)]
struct PlayingFieldFallingState {}

impl PlayingFieldFallingState {
    fn new() -> PlayingFieldFallingState {
        PlayingFieldFallingState {}
    }

    fn handle_input(&self, context: &mut PlayingFieldStateMachineContext, input: Input, elapsed_milliseconds: Duration) {
        let mut timers = context.timers.borrow_mut();
        let mut playing_field_state = context.playing_field_state.borrow_mut();
        match input.kind {
            InputKind::Left => {
                match input.action {
                    InputAction::Press | InputAction::Repeat => {
                        timers.left_hold_timer.update(elapsed_milliseconds);
                        if timers.left_hold_timer.event_triggered() {
                            let collides_with_floor = playing_field_state.collides_with_floor_below();
                            let collides_with_element = playing_field_state.collides_with_element_below();
                            let collides_with_left_element = playing_field_state.collides_with_element_to_the_left();
                            let collides_with_left_wall = playing_field_state.collides_with_left_wall();
                            if !collides_with_left_element || !collides_with_left_wall {
                                if collides_with_floor || collides_with_element {
                                    timers.fall_timer.reset();
                                }
                                playing_field_state.update_block_position(GooglyBlockMove::Left);
                            }
                            timers.left_hold_timer.reset();
                        }
                    }
                    _ => {}
                }
            }
            InputKind::Right => {
                match input.action {
                    InputAction::Press | InputAction::Repeat => {
                        timers.right_hold_timer.update(elapsed_milliseconds);
                        if timers.right_hold_timer.event_triggered() {
                            let collides_with_floor = playing_field_state.collides_with_floor_below();
                            let collides_with_element = playing_field_state.collides_with_element_below();
                            let collides_with_right_element = playing_field_state.collides_with_element_to_the_right();
                            let collides_with_right_wall = playing_field_state.collides_with_right_wall();
                            if !collides_with_right_element || !collides_with_right_wall {
                                if collides_with_floor || collides_with_element {
                                    timers.fall_timer.reset();
                                }
                                playing_field_state.update_block_position(GooglyBlockMove::Right);
                            }
                            timers.right_hold_timer.reset();
                        }
                    }
                    _ => {}
                }
            }
            InputKind::Down => {
                match input.action {
                    InputAction::Press | InputAction::Repeat => {
                        timers.down_hold_timer.update(elapsed_milliseconds);
                        if timers.down_hold_timer.event_triggered() {
                            let collides_with_floor = playing_field_state.collides_with_floor_below();
                            let collides_with_element = playing_field_state.collides_with_element_below();
                            if collides_with_floor || collides_with_element {
                                timers.fall_timer.reset();
                            }
                            playing_field_state.update_block_position(GooglyBlockMove::Down);
                            timers.down_hold_timer.reset();
                        }                        
                    }
                    _ => {}
                }
            }
            InputKind::Rotate => {
                match input.action {
                    InputAction::Press | InputAction::Repeat => {
                        timers.rotate_timer.update(elapsed_milliseconds);
                        if timers.rotate_timer.event_triggered() {
                            playing_field_state.update_block_position(GooglyBlockMove::Rotate);
                            timers.rotate_timer.reset();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        } 
    }

    fn update(&self, context: &mut PlayingFieldStateMachineContext, elapsed_milliseconds: Duration) -> PlayingFieldState {        
        let mut timers = context.timers.borrow_mut();
        let mut playing_field_state = context.playing_field_state.borrow_mut();
        let mut statistics = context.statistics.borrow_mut();
        let mut next_block = context.next_block.borrow_mut();
        let mut full_rows = context.full_rows.borrow_mut();
        let mut flashing_state_machine = context.flashing_state_machine.borrow_mut();

        let collides_with_floor = playing_field_state.collides_with_floor_below();
        let collides_with_element = playing_field_state.collides_with_element_below();

        timers.fall_timer.update(elapsed_milliseconds);
        if collides_with_floor || collides_with_element {
            timers.collision_timer.update(elapsed_milliseconds);
        } else {
            timers.collision_timer.reset();
        }

        if timers.fall_timer.event_triggered() {
            playing_field_state.update_block_position(GooglyBlockMove::Fall);
            timers.fall_timer.reset();
        }

        if timers.collision_timer.event_triggered() {
            let current_block = playing_field_state.current_block;
            playing_field_state.update_landed();
            if !playing_field_state.has_empty_row(0) {
                return PlayingFieldState::GameOver(PlayingFieldGameOverState::new());
            }
            
            statistics.update(current_block);
            let old_next_block = next_block.current_block();
            next_block.update();
            let new_next_block = old_next_block;
            playing_field_state.update_new_block(new_next_block);
            timers.collision_timer.reset();
        }
        
        flashing_state_machine.update(elapsed_milliseconds);

        let full_row_count = playing_field_state.get_full_rows(&mut full_rows.rows);
        full_rows.count = full_row_count;
        if full_row_count > 0 {
            if full_row_count >= 4 {
                flashing_state_machine.enable();
            }
            return PlayingFieldState::Clearing(PlayingFieldClearingState::new());
        } else {
            return PlayingFieldState::Falling(PlayingFieldFallingState::new());
        }
    }
}

#[derive(Copy, Clone)]
struct PlayingFieldClearingState {}

impl PlayingFieldClearingState {
    fn new() -> PlayingFieldClearingState {
        PlayingFieldClearingState {}
    }

    fn handle_input(&self, context: &mut PlayingFieldStateMachineContext, input: Input, elapsed_milliseconds: Duration) {
        match input.kind {
            _ => {}
        }
    }

    fn update(&self, context: &mut PlayingFieldStateMachineContext, elapsed_milliseconds: Duration) -> PlayingFieldState {
        let mut timers = context.timers.borrow_mut();
        let mut playing_field_state = context.playing_field_state.borrow_mut();
        let mut full_rows = context.full_rows.borrow_mut();
        let mut score_board = context.score_board.borrow_mut();
        let mut flashing_state_machine = context.flashing_state_machine.borrow_mut();
        
        timers.clearing_timer.update(elapsed_milliseconds);
        if timers.clearing_timer.event_triggered() {
            timers.clearing_timer.reset();
            let center_left = (4 - context.columns_cleared / 2) as isize;
            let center_right = (5 + context.columns_cleared / 2) as isize;
            for row in full_rows.rows.iter() {
                if *row >= 0 {
                    playing_field_state.landed_blocks.clear(*row, center_left);
                    playing_field_state.landed_blocks.clear(*row, center_right);
                }
            }
            context.columns_cleared += 2;
        }

        flashing_state_machine.update(elapsed_milliseconds);

        if context.columns_cleared >= 10 {
            playing_field_state.collapse_empty_rows();
            score_board.update(full_rows.count);
            full_rows.clear();
            context.columns_cleared = 0;

            return PlayingFieldState::Falling(PlayingFieldFallingState::new());
        }

        PlayingFieldState::Clearing(self.clone())
    }
}

#[derive(Copy, Clone)]
struct PlayingFieldGameOverState {}

impl PlayingFieldGameOverState {
    fn new() -> PlayingFieldGameOverState {
        PlayingFieldGameOverState {}
    }

    fn handle_input(&self, context: &mut PlayingFieldStateMachineContext, input: Input, elapsed_milliseconds: Duration) {
        match input.kind {
            _ => {}
        }
    }

    fn update(&self, context: &mut PlayingFieldStateMachineContext, elapsed_milliseconds: Duration) -> PlayingFieldState {
        let mut flashing_state_machine = context.flashing_state_machine.borrow_mut();
        flashing_state_machine.disable();

        PlayingFieldState::GameOver(*self)
    }
}

enum PlayingFieldState {
    Falling(PlayingFieldFallingState),
    Clearing(PlayingFieldClearingState),
    GameOver(PlayingFieldGameOverState),
}

pub struct PlayingFieldStateMachine {
    context: Rc<RefCell<PlayingFieldStateMachineContext>>,
    state: PlayingFieldState,
}

impl PlayingFieldStateMachine {
    fn new(context: Rc<RefCell<PlayingFieldStateMachineContext>>) -> PlayingFieldStateMachine {
        PlayingFieldStateMachine {
            context: context,
            state: PlayingFieldState::Falling(PlayingFieldFallingState::new()),
        }
    }

    pub fn flashing_state_machine(&self) -> Rc<RefCell<FlashAnimationStateMachine>> {
        self.context.borrow().flashing_state_machine.clone()
    }

    pub fn is_game_over(&self) -> bool {
        match self.state {
            PlayingFieldState::GameOver(_) => true,
            _ => false,
        }
    }

    pub fn handle_input(&self, input: Input, elapsed_milliseconds: Duration) {
        let mut context = self.context.borrow_mut();
        match self.state {
            PlayingFieldState::Falling(s) => s.handle_input(&mut context, input, elapsed_milliseconds),
            PlayingFieldState::Clearing(s) => s.handle_input(&mut context, input, elapsed_milliseconds),
            PlayingFieldState::GameOver(s) => s.handle_input(&mut context, input, elapsed_milliseconds),
        }
    }

    pub fn update(&mut self, elapsed_milliseconds: Duration) {
        let mut context = self.context.borrow_mut();
        self.state = match self.state {
            PlayingFieldState::Falling(s) => s.update(&mut context, elapsed_milliseconds),
            PlayingFieldState::Clearing(s) => s.update(&mut context, elapsed_milliseconds),
            PlayingFieldState::GameOver(s) => s.update(&mut context, elapsed_milliseconds),
        };
    }
}

