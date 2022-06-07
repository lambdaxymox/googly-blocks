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
 

/// The element making up a googly block. This is the set of elements
/// composing a googly block for display on the screen.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum GooglyBlockElement {
    /// Empty space.
    EmptySpace,
    /// The element of a T piece.
    T,
    /// The element of a J piece.
    J,
    /// The element of a Z piece.
    Z,
    /// The element of a O piece.
    O,
    /// The element of a S piece.
    S,
    /// The element of a L piece.
    L,
    /// The element of a I piece.
    I,
}
 
impl GooglyBlockElement {
    /// Determine whether a googly block element is empty space.
    #[inline]
    pub fn is_empty(self) -> bool {
        self == GooglyBlockElement::EmptySpace
    }
 
    /// Determine whether an element is the element of a googly
    /// block piece, and not empty space.
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

/// The data needed for moving a googly block around in the playing field.
pub struct GooglyBlockShape {
    /// The element that composes the occupied (non-empty space) googly block
    /// cells.
    pub element: GooglyBlockElement,
    /// The wall kick distance is the distance a particular shape
    /// will move away from the left wall or the right wall of the playing field
    /// when we rotate the block.
    pub wall_kick_distance: isize,
    /// The height of shape in playing field cells.
    rows: usize,
    /// The width of the shape in playing field cells.
    columns: usize,
    /// The placement of the non-empty cells of the shape.
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
 
/// The set of possible rotations for a googly block. Each 
/// rotation corresponds to a different way to place the piece in
/// the playing field.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum GooglyBlockRotation {
    R0,
    R1,
    R2,
    R3,
}

/// This sum type represents the kind of block a particular googly block is.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum GooglyBlockPiece {
    /// T block.
    T,
    /// J block.
    J,
    /// Z block.
    Z,
    /// O block.
    O,
    /// S block.
    S,
    /// L block.
    L,
    /// I block.
    I,
}

/// A googly block consists of two parts: A piece, the kind of block that it is,
/// and a rotation, which is the orientation of the piece in the playing field.
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
 
    /// Get the shape data for a particular googly block.
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

    /// Generate the a googly block corresponding to the same piece rotated counter-clockwise
    /// in the playing field.
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
