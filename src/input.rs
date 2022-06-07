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
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InputAction {
    Press,
    Repeat,
    Release,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InputKind {
    Left,
    Right,
    Down,
    Exit,
    Rotate,
    StartGame,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Input {
    pub kind: InputKind,
    pub action: InputAction,
}

impl Input {
    #[inline]
    pub fn new(kind: InputKind, action: InputAction) -> Input {
        Input {
            kind: kind,
            action: action,
        }
    }
}

