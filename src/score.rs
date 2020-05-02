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
use playing_field::{
    GooglyBlockPiece,
    GooglyBlock,
};

/// The score board type that tracks the player's progress during a 
/// game of Googly Blocks.
pub struct ScoreBoard {
    /// The player's score.
    pub score: usize,
    /// The player's level.
    pub level: usize,
    /// The number of lines cleared.
    pub lines: usize,
    /// The number of times the maximum possible number of lines
    /// was cleared with one piece. In particular, when four lines
    /// are cleared with an I piece.
    pub tetrises: usize,
    /// The number of lines left before the next level.
    lines_before_next_level: usize,
    /// The number of lines per level.
    lines_per_level: usize,
}

impl ScoreBoard {
    /// Construct a new scoreboard.
    pub fn new(lines_per_level: usize) -> ScoreBoard {
        ScoreBoard {
            score: 0,
            level: 0,
            lines: 0,
            tetrises: 0,
            lines_before_next_level: lines_per_level,
            lines_per_level: lines_per_level,
        }
    }

    /// Update the scoreboard.
    pub fn update(&mut self, new_lines_cleared: usize) {
        self.lines += new_lines_cleared;
        if new_lines_cleared >= self.lines_before_next_level {
            self.level += 1;
            self.lines_before_next_level = self.lines_per_level;
        } else {
            self.lines_before_next_level -= new_lines_cleared;
        }

        match new_lines_cleared {
            0 => {}
            1 => {
                self.score += 40;
            }
            2 => {
                self.score += 100;
            }
            3 => {
                self.score += 300;
            }
            _ => {
                self.score += 1200;
                self.tetrises += 1;
            }
        }
    }
}

pub struct Statistics {
    pub t_pieces: usize,
    pub j_pieces: usize,
    pub z_pieces: usize,
    pub o_pieces: usize,
    pub s_pieces: usize,
    pub l_pieces: usize,
    pub i_pieces: usize, 
}

impl Statistics {
    pub fn new() -> Statistics {
        Statistics {
            t_pieces: 0,
            j_pieces: 0,
            z_pieces: 0,
            o_pieces: 0,
            s_pieces: 0,
            l_pieces: 0,
            i_pieces: 0,
        }
    }

    pub fn update(&mut self, block: GooglyBlock) {
        match block.piece {
            GooglyBlockPiece::T => self.t_pieces += 1,
            GooglyBlockPiece::J => self.j_pieces += 1,
            GooglyBlockPiece::Z => self.z_pieces += 1,
            GooglyBlockPiece::O => self.o_pieces += 1,
            GooglyBlockPiece::S => self.s_pieces += 1,
            GooglyBlockPiece::L => self.l_pieces += 1,
            GooglyBlockPiece::I => self.i_pieces += 1,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::ScoreBoard;


    /// The score board should increment the game level after the number of lines
    /// specified at construction time have been cleared.
    #[test]
    fn score_board_should_transition_to_next_level_on_crossing_line_threshold() {
        let mut score_board = ScoreBoard::new(20);
        score_board.update(20);
        let expected = 1;
        let result = score_board.level;

        assert_eq!(result, expected);
    }

    /// The score board should not increment the game level until the number of lines cleared meets or exceeds
    /// the number of lines per level.
    #[test]
    fn score_board_should_not_transition_to_next_level_if_lines_per_level_not_crossed() {
        let mut score_board = ScoreBoard::new(20);
        score_board.update(19);
        let expected = 0;
        let result = score_board.level;

        assert_eq!(result, expected);
    }

    /// The number lines for the next level should be less than or equal to the lines per level
    /// after a level transition.
    #[test]
    fn score_board_lines_before_next_level_should_not_exceed_lines_per_level() {
        let mut score_board = ScoreBoard::new(20);
        score_board.update(21);
        let expected = score_board.lines_per_level;
        let result = score_board.lines_before_next_level;

        assert!(result <= expected);
    }
}