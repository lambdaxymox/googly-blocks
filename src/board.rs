/*
 *  Googly Blocks is a video game.
 *  Copyright (C) 2018,2019,2029  Christopher Blanchard
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
use std::fmt::Write;
use std::ops;


#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
enum GooglyBlockElement {
    EmptySpace,
    T,
    J,
    Z,
    O,
    S,
    L,
    I,
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

struct GooglyBlockShape {
    shape: [(usize, usize); 4],
    element: GooglyBlockElement,
    rows: usize,
    columns: usize,
}

impl fmt::Display for GooglyBlockShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut shape_matrix = [[GooglyBlockElement::EmptySpace; 4]; 4];
        for (row, column) in self.shape.iter() {
            shape_matrix[*row][*column] = self.element;
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
enum Rotation {
    R0,
    R1,
    R2,
    R3,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
enum GooglyBlockPiece {
    T,
    J,
    Z,
    O,
    S,
    L,
    I,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
struct GooglyBlock {
    piece: GooglyBlockPiece,
    rotation: Rotation,
}

impl GooglyBlock {
    fn shape(&self) -> GooglyBlockShape {
        use self::GooglyBlockPiece::*;
        use self::Rotation::*;
        match self.piece {
            T => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(0, 0), (0, 1), (0, 2), (1, 1)],
                    element: GooglyBlockElement::T,
                    rows: 3,
                    columns: 3,
                },
                R1 => GooglyBlockShape {
                    shape: [(1, 0), (0, 1), (1, 1), (2, 1)],
                    element: GooglyBlockElement::T,
                    rows: 3,
                    columns: 3,
                },
                R2 => GooglyBlockShape {
                    shape: [(0, 1), (1, 0), (1, 1), (1, 2)],
                    element: GooglyBlockElement::T,
                    rows: 3,
                    columns: 3,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 1), (1, 1), (1, 2), (2, 1)],
                    element: GooglyBlockElement::T,
                    rows: 3,
                    columns: 3,
                },
            }
            J => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(1, 0), (1, 1), (1, 2), (2, 2)],
                    element: GooglyBlockElement::J,
                    rows: 3,
                    columns: 3,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 1), (1, 1), (2, 1), (2, 0)],
                    element: GooglyBlockElement::J,
                    rows: 3,
                    columns: 3,
                },
                R2 => GooglyBlockShape {
                    shape: [(0, 0), (1, 0), (1, 1), (1, 2)],
                    element: GooglyBlockElement::J,
                    rows: 3,
                    columns: 3,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 1), (0, 2), (1, 1), (2, 1)],
                    element: GooglyBlockElement::J,
                    rows: 3,
                    columns: 3,
                },
            }
            Z => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(1, 0), (1, 1), (2, 1), (2, 2)],
                    element: GooglyBlockElement::Z,
                    rows: 3,
                    columns: 3,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 2), (1, 1), (1, 2), (2, 1)],
                    element: GooglyBlockElement::Z,
                    rows: 3,
                    columns: 3,
                },
                R2 => GooglyBlockShape {
                    shape: [(1, 0), (1, 1), (2, 1), (2, 2)],
                    element: GooglyBlockElement::Z,
                    rows: 3,
                    columns: 3,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 2), (1, 1), (1, 2), (2, 1)],
                    element: GooglyBlockElement::Z,
                    rows: 3,
                    columns: 3,
                },        
            }
            O => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(1, 1), (1, 2), (2, 1), (2, 2)],
                    element: GooglyBlockElement::O,
                    rows: 4,
                    columns: 4,
                },
                R1 => GooglyBlockShape {
                    shape: [(1, 1), (1, 2), (2, 1), (2, 2)],
                    element: GooglyBlockElement::O,
                    rows: 4,
                    columns: 4,
                },
                R2 => GooglyBlockShape {
                    shape: [(1, 1), (1, 2), (2, 1), (2, 2)],
                    element: GooglyBlockElement::O,
                    rows: 4,
                    columns: 4,
                },
                R3 => GooglyBlockShape {
                    shape: [(1, 1), (1, 2), (2, 1), (2, 2)],
                    element: GooglyBlockElement::O,
                    rows: 4,
                    columns: 4,
                },              
            }
            S => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(1, 1), (1, 2), (2, 0), (2, 1)],
                    element: GooglyBlockElement::S,
                    rows: 3,
                    columns: 3,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 1), (1, 1), (1, 2), (2, 2)],
                    element: GooglyBlockElement::S,
                    rows: 3,
                    columns: 3,
                },
                R2 => GooglyBlockShape {
                    shape: [(1, 1), (1, 2), (2, 0), (2, 1)],
                    element: GooglyBlockElement::S,
                    rows: 3,
                    columns: 3,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 1), (1, 1), (1, 2), (2, 2)],
                    element: GooglyBlockElement::S,
                    rows: 3,
                    columns: 3,
                },            
            }
            L => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(1, 0), (1, 1), (1, 2), (2, 0)],
                    element: GooglyBlockElement::L,
                    rows: 3,
                    columns: 3,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 0), (0, 1), (1, 1), (2, 1)],
                    element: GooglyBlockElement::L,
                    rows: 3,
                    columns: 3,
                },
                R2 => GooglyBlockShape {
                    shape: [(0, 2), (1, 0), (1, 1), (1, 2)],
                    element: GooglyBlockElement::L,
                    rows: 3,
                    columns: 3,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 1), (1, 1), (2, 1), (2, 2)],
                    element: GooglyBlockElement::L,
                    rows: 3,
                    columns: 3,
                },             
            }
            I => match self.rotation {
                R0 => GooglyBlockShape {
                    shape: [(2, 0), (2, 1), (2, 2), (2, 3)],
                    element: GooglyBlockElement::I,
                    rows: 4,
                    columns: 4,
                },
                R1 => GooglyBlockShape {
                    shape: [(0, 2), (1, 2), (2, 2), (3, 2)],
                    element: GooglyBlockElement::I,
                    rows: 4,
                    columns: 4,
                },
                R2 => GooglyBlockShape {
                    shape: [(2, 0), (2, 1), (2, 2), (2, 3)],
                    element: GooglyBlockElement::I,
                    rows: 4,
                    columns: 4,
                },
                R3 => GooglyBlockShape {
                    shape: [(0, 2), (1, 2), (2, 2), (3, 2)],
                    element: GooglyBlockElement::I,
                    rows: 4,
                    columns: 4,
                },           
            }
        }
    }
}

impl fmt::Display for GooglyBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", self.shape())
    }   
}


#[derive(Copy, Clone, Debug)]
struct LandedBlocksRow {
    row: [GooglyBlockElement; 10],
}

impl LandedBlocksRow {
    fn new() -> LandedBlocksRow {
        LandedBlocksRow {
            row: [GooglyBlockElement::EmptySpace; 10],
        }
    }

    fn len(&self) -> usize { 10 }
}

impl ops::Index<usize> for LandedBlocksRow {
    type Output = GooglyBlockElement;

    fn index(&self, idx: usize) -> &Self::Output {
        &self.row[idx]
    }
}

impl ops::IndexMut<usize> for LandedBlocksRow {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.row[idx]
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum LandedBlocksQuery {
    InOfBounds(GooglyBlockElement),
    OutOfBounds(isize, isize),
}

impl LandedBlocksQuery {
    fn is_in_of_bounds(&self) -> bool {
        match *self {
            LandedBlocksQuery::InOfBounds(_) => true,
            _ => false,
        }
    }

    fn is_out_of_bounds(&self) -> bool {
        match *self {
            LandedBlocksQuery::OutOfBounds(_,_) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
struct LandedBlocks {
    landed: [LandedBlocksRow; 20],
}

impl LandedBlocks {
    fn new() -> Self {
        LandedBlocks {
            landed: [LandedBlocksRow { row: [GooglyBlockElement::EmptySpace; 10] }; 20],
        }
    }

    fn get(&self, row: isize, column: isize) -> LandedBlocksQuery {
        let rows = self.rows() as isize;
        let columns = self.columns() as isize;
        if row < 0 || row >= rows || column < 0 || column >= columns {
            LandedBlocksQuery::OutOfBounds(row, column)
        } else {
            LandedBlocksQuery::InOfBounds(self.landed[row as usize][column as usize])
        }
    }

    fn insert(&mut self, row: isize, column: isize, element: GooglyBlockElement) {
        let rows = self.rows() as isize;
        let columns = self.columns() as isize;
        if row >= 0 && row < rows && column >= 0 && column < columns {
            let row_idx = row as usize;
            let column_idx = column as usize;
            self.landed[row_idx][column_idx] = element;
        }
    }

    fn insert_block(&mut self, tl_row: isize, tl_column: isize, block: GooglyBlock) {
        let shape = block.shape();
        for (row, column) in shape.shape.iter().map(|(r, c)| (*r as isize, *c as isize)) {
            self.insert(tl_row + row, tl_column + column, shape.element);
        }
    }

    #[inline]
    fn rows(&self) -> usize { 20 }

    #[inline]
    fn columns(&self) -> usize { 10 }
}

impl fmt::Display for LandedBlocks {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {    
        let mut disp = format!("{}", "");
        for row in 0..self.rows() {
            disp.push_str("| ");
            for column in 0..self.columns() {
                disp.push_str(&format!("{} ", self.landed[row][column]));
            }
            disp.push_str("|\n");
        }
        disp.push_str("|=====================|");

        write!(f, "{}", disp)
    }

}

impl ops::Index<usize> for LandedBlocks {
    type Output = [GooglyBlockElement];
    
    fn index(&self, idx: usize) -> &Self::Output {
        &self.landed[idx].row
    }
}

impl ops::IndexMut<usize> for LandedBlocks {    
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.landed[idx].row
    }
}

#[cfg(test)]
mod landed_blocks_tests {
    use super::{GooglyBlockElement, LandedBlocks, LandedBlocksQuery};
    #[test]
    fn inserting_an_element_and_getting_it_back_yields_the_same_element() {
        let element = GooglyBlockElement::J;
        let mut landed = LandedBlocks::new();
        landed.insert(4, 6, element);

        let expected = LandedBlocksQuery::InOfBounds(element);
        let result = landed.get(4, 6);
        assert_eq!(expected, result);
    }
}