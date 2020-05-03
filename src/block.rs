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
use std::fmt;
use std::iter::Iterator;
 
 
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum GooglyBlockElement {
    EmptySpace,
    T,
    J,
    Z,
    O,
    S,
    L,
    I,
}
 
impl GooglyBlockElement {
    #[inline]
    pub fn is_empty(self) -> bool {
        self == GooglyBlockElement::EmptySpace
    }
 
    #[inline]
    pub fn is_not_empty(self) -> bool {
        self != GooglyBlockElement::EmptySpace
    }
}
 
impl fmt::Display for GooglyBlockElement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::GooglyBlockElement::*;
        let disp = match *self {
            EmptySpace => "#",
            T => "T",
            J => "J",
            Z => "Z",
            O => "O",
            S => "S",
            L => "L",
            I => "I",
        };
 
        write!(f, "{}", disp)
    }
}
 
pub struct GooglyBlockShape {
    pub element: GooglyBlockElement,
    pub wall_kick_distance: isize,
    rows: usize,
    columns: usize,
    shape: [(usize, usize); 4],
}
 
pub struct GooglyBlockShapeIterator<'a> {
    index: usize,
    shape: &'a GooglyBlockShape
}
 
impl<'a> Iterator for GooglyBlockShapeIterator<'a> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < 4 {
            let item = self.shape.shape[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}
 
impl GooglyBlockShape {
    pub fn iter(&self) -> GooglyBlockShapeIterator {
        GooglyBlockShapeIterator {
            index: 0,
            shape: self,
        }
    }
}
 
impl fmt::Display for GooglyBlockShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut shape_matrix = [[GooglyBlockElement::EmptySpace; 4]; 4];
        for (row, column) in self.iter() {
            shape_matrix[row][column] = self.element;
        }
         
        let mut disp = format!("{}", "");
        for row in 0..self.rows {
            disp.push_str("[ ");
            for column in 0..self.columns {
                disp.push_str(&format!("{} ", shape_matrix[row][column]));
            }
            disp.push_str("]\n");
        }
 
        write!(f, "{}", disp)
    }
}
 
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum GooglyBlockRotation {
    R0,
    R1,
    R2,
    R3,
}
 
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum GooglyBlockPiece {
    T,
    J,
    Z,
    O,
    S,
    L,
    I,
}
 
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct GooglyBlock {
    pub piece: GooglyBlockPiece,
    rotation: GooglyBlockRotation,
}
 
impl GooglyBlock {
    #[inline]
    pub fn new(piece: GooglyBlockPiece, rotation: GooglyBlockRotation) -> GooglyBlock {
        GooglyBlock {
            piece: piece,
            rotation: rotation,
        }
    }
 
    pub fn shape(&self) -> GooglyBlockShape {
        use self::GooglyBlockPiece::*;
        use self::GooglyBlockRotation::*;
        match self.piece {
            T => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(0, 0), (0, 1), (0, 2), (1, 1)],
                    element: GooglyBlockElement::T,
                    wall_kick_distance: 1,
                    rows: 3,
                    columns: 3,
                },
                R1 => GooglyBlockShape {
                    shape: [(1, 0), (0, 1), (1, 1), (2, 1)],
                    element: GooglyBlockElement::T,
                    wall_kick_distance: 0,
                    rows: 3,
                    columns: 3,
                },
                R2 => GooglyBlockShape {
                    shape: [(0, 1), (1, 0), (1, 1), (1, 2)],
                    element: GooglyBlockElement::T,
                    wall_kick_distance: 1,
                    rows: 3,
                    columns: 3,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 1), (1, 1), (1, 2), (2, 1)],
                    element: GooglyBlockElement::T,
                    wall_kick_distance: 0,
                    rows: 3,
                    columns: 3,
                },
            }
            J => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(1, 0), (1, 1), (1, 2), (2, 2)],
                    element: GooglyBlockElement::J,
                    wall_kick_distance: 1,
                    rows: 3,
                    columns: 3,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 1), (1, 1), (2, 1), (2, 0)],
                    element: GooglyBlockElement::J,
                    wall_kick_distance: 0,
                    rows: 3,
                    columns: 3,
                },
                R2 => GooglyBlockShape {
                    shape: [(0, 0), (1, 0), (1, 1), (1, 2)],
                    element: GooglyBlockElement::J,
                    wall_kick_distance: 1,
                    rows: 3,
                    columns: 3,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 1), (0, 2), (1, 1), (2, 1)],
                    element: GooglyBlockElement::J,
                    wall_kick_distance: 0,
                    rows: 3,
                    columns: 3,
                },
            }
            Z => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(1, 0), (1, 1), (2, 1), (2, 2)],
                    element: GooglyBlockElement::Z,
                    wall_kick_distance: 1,
                    rows: 3,
                    columns: 3,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 2), (1, 1), (1, 2), (2, 1)],
                    element: GooglyBlockElement::Z,
                    wall_kick_distance: 0,
                    rows: 3,
                    columns: 3,
                },
                R2 => GooglyBlockShape {
                    shape: [(1, 0), (1, 1), (2, 1), (2, 2)],
                    element: GooglyBlockElement::Z,
                    wall_kick_distance: 1,
                    rows: 3,
                    columns: 3,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 2), (1, 1), (1, 2), (2, 1)],
                    element: GooglyBlockElement::Z,
                    wall_kick_distance: 0,
                    rows: 3,
                    columns: 3,
                },        
            }
            O => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(0, 0), (0, 1), (1, 0), (1, 1)],
                    element: GooglyBlockElement::O,
                    wall_kick_distance: 0,
                    rows: 2,
                    columns: 2,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 0), (0, 1), (1, 0), (1, 1)],
                    element: GooglyBlockElement::O,
                    wall_kick_distance: 0,
                    rows: 2,
                    columns: 2,
                },
                R2 => GooglyBlockShape {
                    shape: [(0, 0), (0, 1), (1, 0), (1, 1)],
                    element: GooglyBlockElement::O,
                    wall_kick_distance: 0,
                    rows: 2,
                    columns: 2,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 0), (0, 1), (1, 0), (1, 1)],
                    element: GooglyBlockElement::O,
                    wall_kick_distance: 0,
                    rows: 2,
                    columns: 2,
                },              
            }
            S => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(1, 1), (1, 2), (2, 0), (2, 1)],
                    element: GooglyBlockElement::S,
                    wall_kick_distance: 1,
                    rows: 3,
                    columns: 3,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 1), (1, 1), (1, 2), (2, 2)],
                    element: GooglyBlockElement::S,
                    wall_kick_distance: 0,
                    rows: 3,
                    columns: 3,
                },
                R2 => GooglyBlockShape {
                    shape: [(1, 1), (1, 2), (2, 0), (2, 1)],
                    element: GooglyBlockElement::S,
                    wall_kick_distance: 1,
                    rows: 3,
                    columns: 3,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 1), (1, 1), (1, 2), (2, 2)],
                    element: GooglyBlockElement::S,
                    wall_kick_distance: 0,
                    rows: 3,
                    columns: 3,
                },            
            }
            L => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(1, 0), (1, 1), (1, 2), (2, 0)],
                    element: GooglyBlockElement::L,
                    wall_kick_distance: 1,
                    rows: 3,
                    columns: 3,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 0), (0, 1), (1, 1), (2, 1)],
                    element: GooglyBlockElement::L,
                    wall_kick_distance: 0,
                    rows: 3,
                    columns: 3,
                },
                R2 => GooglyBlockShape {
                    shape: [(0, 2), (1, 0), (1, 1), (1, 2)],
                    element: GooglyBlockElement::L,
                    wall_kick_distance: 1,
                    rows: 3,
                    columns: 3,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 1), (1, 1), (2, 1), (2, 2)],
                    element: GooglyBlockElement::L,
                    wall_kick_distance: 0,
                    rows: 3,
                    columns: 3,
                },             
            }
            I => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(2, 0), (2, 1), (2, 2), (2, 3)],
                    element: GooglyBlockElement::I,
                    wall_kick_distance: 2,
                    rows: 4,
                    columns: 4,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 2), (1, 2), (2, 2), (3, 2)],
                    element: GooglyBlockElement::I,
                    wall_kick_distance: 0,
                    rows: 4,
                    columns: 4,
                },
                R2 => GooglyBlockShape {
                    shape: [(2, 0), (2, 1), (2, 2), (2, 3)],
                    element: GooglyBlockElement::I,
                    wall_kick_distance: 2,
                    rows: 4,
                    columns: 4,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 2), (1, 2), (2, 2), (3, 2)],
                    element: GooglyBlockElement::I,
                    wall_kick_distance: 0,
                    rows: 4,
                    columns: 4,
                },           
            }
        }
    }

    pub fn rotate(&self) -> GooglyBlock {
        match self.rotation {
            GooglyBlockRotation::R0 => GooglyBlock::new(self.piece, GooglyBlockRotation::R1),
            GooglyBlockRotation::R1 => GooglyBlock::new(self.piece, GooglyBlockRotation::R2),
            GooglyBlockRotation::R2 => GooglyBlock::new(self.piece, GooglyBlockRotation::R3),
            GooglyBlockRotation::R3 => GooglyBlock::new(self.piece, GooglyBlockRotation::R0),
        }
    }
}

impl fmt::Display for GooglyBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", self.shape())
    }   
}


#[cfg(test)]
mod tests {
    use super::{
        GooglyBlockPiece,
        GooglyBlockRotation,
        GooglyBlock,
    };


    /// Given a googly block, if we rotate it four times, it should cycle through all of its rotations. 
    /// That is, the last rotation should be the original rotation state.
    #[test]
    fn googly_block_rotations_should_cycle() {
        let expected = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
        let result = expected.rotate().rotate().rotate().rotate();

        assert_eq!(result, expected);
    }
}
