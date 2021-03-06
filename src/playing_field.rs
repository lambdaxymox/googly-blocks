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
use crate::block::{
    GooglyBlock, 
    GooglyBlockPiece, 
    GooglyBlockElement,
};
use std::fmt;
use std::iter::Iterator;
use std::ops;
use std::collections::hash_map::HashMap;


#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LandedBlocksQuery {
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

    fn is_empty_space(&self) -> bool {
        match *self {
            LandedBlocksQuery::InOfBounds(GooglyBlockElement::EmptySpace) => true,
            _ => false,
        }
    }

    pub fn unwrap(&self) -> GooglyBlockElement {
        match *self {
            LandedBlocksQuery::InOfBounds(element) => element,
            _ => panic!("Queried a playing field cell that was out of bounds."),
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct LandedBlocksGridRow {
    inner: [GooglyBlockElement; 10],
    occupied: usize,
}

impl LandedBlocksGridRow {
    fn new() -> LandedBlocksGridRow {
        LandedBlocksGridRow {
            inner: [GooglyBlockElement::EmptySpace; 10],
            occupied: 0,
        }
    }

    fn clear(&mut self) {
        for i in 0..self.len() {
            self.inner[i] = GooglyBlockElement::EmptySpace;
        }

        self.occupied = 0;
    }

    #[inline]
    fn len(&self) -> usize { 10 }

    #[inline]
    fn is_full(&self) -> bool {
        self.occupied == self.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.occupied == 0
    }
}

impl ops::Index<usize> for LandedBlocksGridRow {
    type Output = GooglyBlockElement;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}

impl ops::IndexMut<usize> for LandedBlocksGridRow {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.inner[index]
    }
}

#[derive(Clone, Debug)]
pub struct LandedBlocksGrid {
    landed: [LandedBlocksGridRow; 20],
}

struct LandedBlocksIterator {
    row: usize,
    column: usize,
    rows: usize,
    columns: usize,
}

impl Iterator for LandedBlocksIterator {
    type Item = (isize, isize);

    fn next(&mut self) -> Option<Self::Item> {
        if (self.row < self.rows) && (self.column < self.columns) {
            let item = (self.row as isize, self.column as isize);
            self.column += 1;
            if self.column >= self.columns {
                self.row += 1;
                self.column = 0;
            }

            Some(item)
        } else {
            None
        }
    }
}

struct LandedBlocksRowIterator<'a> {
    row: usize,
    landed_blocks: &'a LandedBlocksGrid,
}

impl<'a> Iterator for LandedBlocksRowIterator<'a> {
    type Item = (usize, &'a LandedBlocksGridRow);

    fn next(&mut self) -> Option<Self::Item> {
        if self.row < self.landed_blocks.rows() {
            let item = (self.row, &self.landed_blocks.landed[self.row]);
            self.row += 1;

            Some(item)
        } else {
            None
        }
    }
}

impl LandedBlocksGrid {
    pub fn new() -> Self {
        LandedBlocksGrid {
            landed: [LandedBlocksGridRow::new(); 20],
        }
    }

    pub fn get(&self, row: isize, column: isize) -> LandedBlocksQuery {
        let rows = self.rows() as isize;
        let columns = self.columns() as isize;
        if row < 0 || row >= rows || column < 0 || column >= columns {
            LandedBlocksQuery::OutOfBounds(row, column)
        } else {
            LandedBlocksQuery::InOfBounds(self.landed[row as usize][column as usize])
        }
    }

    pub fn insert(&mut self, row: isize, column: isize, new_element: GooglyBlockElement) {
        let rows = self.rows() as isize;
        let columns = self.columns() as isize;
        if (row >= 0) && (row < rows) && (column >= 0) && (column < columns) {
            let row_idx = row as usize;
            let column_idx = column as usize;
            let old_element = self.landed[row_idx][column_idx];
            if old_element.is_empty() && new_element.is_not_empty() {
                self.landed[row_idx].occupied += 1;
            } else if old_element.is_not_empty() && new_element.is_empty() {
                self.landed[row_idx].occupied -= 1;
            }
            self.landed[row_idx][column_idx] = new_element;
        }
    }

    pub fn clear(&mut self, row: isize, column: isize) {
        self.insert(row, column, GooglyBlockElement::EmptySpace);
    }

    pub fn insert_block(&mut self, tl_row: isize, tl_column: isize, block: GooglyBlock) {
        let shape = block.shape();
        for (row, column) in shape.iter().map(|(r, c)| (r as isize, c as isize)) {
            self.insert(tl_row + row, tl_column + column, shape.element);
        }
    }

    #[inline]
    pub fn has_empty_row(&self, row: isize) -> bool {
        if row >= 0 { 
            let row_query = row as usize;
            if row_query < self.rows() {
                return self.landed[row_query].is_empty();
            }
        }

        false
    }

    #[inline]
    pub fn rows(&self) -> usize { 20 }

    #[inline]
    pub fn columns(&self) -> usize { 
        self.landed[0].len()
    }

    fn iter(&self) -> LandedBlocksIterator {
        LandedBlocksIterator {
            row: 0,
            column: 0,
            rows: self.rows(),
            columns: self.columns(),
        }
    }

    fn row_iter(&self) -> LandedBlocksRowIterator {
        LandedBlocksRowIterator {
            landed_blocks: self,
            row: 0,
        }
    }
}

impl fmt::Display for LandedBlocksGrid {
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BlockPosition {
    pub row: isize,
    pub column: isize,
}

impl BlockPosition {
    #[inline]
    pub fn new(row: isize, column: isize) -> BlockPosition {
        BlockPosition {
            row: row,
            column: column,
        }
    }
}


fn collides_with_element(piece: GooglyBlock, top_left: BlockPosition, landed: &LandedBlocksGrid) -> bool {
    let shape = piece.shape();
    for (row, column) in shape.iter() {
        let element_row = row as isize;
        let element_column = column as isize;
        match landed.get(top_left.row + element_row, top_left.column + element_column) {
            LandedBlocksQuery::InOfBounds(GooglyBlockElement::EmptySpace) => {}
            LandedBlocksQuery::OutOfBounds(_, _) => {}
            LandedBlocksQuery::InOfBounds(_) => return true,
        }
    }
    
    false
}

fn collides_with_left_wall(piece: GooglyBlock, top_left: BlockPosition, landed: &LandedBlocksGrid) -> bool {
    let shape = piece.shape();
    for (_, column) in shape.iter() {
        let element_column = column as isize;
        if top_left.column + element_column < 0 {
            return true;
        }
    }

    false
}

fn collides_with_right_wall(piece: GooglyBlock, top_left: BlockPosition, landed: &LandedBlocksGrid) -> bool {
    let shape = piece.shape();
    for (_, column) in shape.iter() {
        let element_column = column as isize;
        if top_left.column + element_column >= landed.columns() as isize {
            return true;
        }
    }

    false
}

fn collides_with_floor(piece: GooglyBlock, top_left: BlockPosition, landed: &LandedBlocksGrid) -> bool {
    let shape = piece.shape();
    for (row, _) in shape.iter() {
        let part_row = row as isize;
        if top_left.row + part_row >= landed.rows() as isize {
            return true;
        }
    }

    false
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum GooglyBlockMove {
    Left,
    Right,
    Down,
    Fall,
    Rotate,
}

pub struct PlayingFieldContextSpec {
    pub starting_block: GooglyBlock,
    pub starting_positions: HashMap<GooglyBlockPiece, BlockPosition>,
}

pub struct PlayingFieldContext {
    pub current_block: GooglyBlock,
    pub current_position: BlockPosition,
    pub landed_blocks: LandedBlocksGrid,
    starting_positions: HashMap<GooglyBlockPiece, BlockPosition>,
}

impl PlayingFieldContext {
    pub fn new(spec: PlayingFieldContextSpec) -> PlayingFieldContext {
        PlayingFieldContext {
            current_block: spec.starting_block,
            current_position: spec.starting_positions[&spec.starting_block.piece],
            landed_blocks: LandedBlocksGrid::new(),
            starting_positions: spec.starting_positions,
        }
    }

    pub fn get_full_rows(&self, out: &mut [isize]) -> usize {
        let mut full_row_count = 0;
        for (i, row_i) in self.landed_blocks.row_iter() {
            if row_i.is_full() {
                out[full_row_count] = i as isize;
                full_row_count += 1;
            }
        }

        full_row_count
    }

    pub fn has_empty_row(&self, row: isize) -> bool {
        self.landed_blocks.has_empty_row(row)
    }
    
    pub fn update_block_position(&mut self, block_move: GooglyBlockMove) {
        match block_move {
            GooglyBlockMove::Fall => {
                let potential_top_left = BlockPosition::new(self.current_position.row + 1, self.current_position.column);
                let collides_with_element = collides_with_element(self.current_block, potential_top_left, &self.landed_blocks);
                let collides_with_floor = collides_with_floor(self.current_block, potential_top_left, &self.landed_blocks);
                if !(collides_with_element || collides_with_floor) {
                    self.current_position = potential_top_left;
                } 
            }
            GooglyBlockMove::Right => {
                let potential_top_left = BlockPosition::new(self.current_position.row, self.current_position.column + 1);
                let collides_with_element = collides_with_element(self.current_block, potential_top_left, &self.landed_blocks);
                let collides_with_right_wall = collides_with_right_wall(self.current_block, potential_top_left, &self.landed_blocks);
                if !(collides_with_element || collides_with_right_wall) {
                    self.current_position = potential_top_left;
                } 
            }
            GooglyBlockMove::Left => {
                let potential_top_left = BlockPosition::new(self.current_position.row, self.current_position.column - 1);
                let collides_with_element = collides_with_element(self.current_block, potential_top_left, &self.landed_blocks);
                let collides_with_right_wall = collides_with_left_wall(self.current_block, potential_top_left, &self.landed_blocks);
                if !(collides_with_element || collides_with_right_wall) {
                    self.current_position = potential_top_left;
                }             
            }
            GooglyBlockMove::Down => {
                let potential_top_left = BlockPosition::new(self.current_position.row + 1, self.current_position.column);
                let collides_with_element = collides_with_element(self.current_block, potential_top_left, &self.landed_blocks);
                let collides_with_floor = collides_with_floor(self.current_block, potential_top_left, &self.landed_blocks);
                if !(collides_with_element || collides_with_floor) {
                    self.current_position = potential_top_left;
                } 
            }
            GooglyBlockMove::Rotate => {
                let potential_top_left = self.current_position;
                let potential_block = self.current_block.rotate();
                let potential_block_shape = potential_block.shape();

                let collides_with_left_wall = collides_with_left_wall(potential_block, potential_top_left, &self.landed_blocks);
                let collides_with_right_wall = collides_with_right_wall(potential_block, potential_top_left, &self.landed_blocks);
                if collides_with_left_wall {
                    let potential_top_left = BlockPosition::new(
                        potential_top_left.row, potential_top_left.column + potential_block_shape.wall_kick_distance
                    );
                    let collides_with_element = collides_with_element(potential_block, potential_top_left, &self.landed_blocks);
                    let collides_with_floor = collides_with_floor(potential_block, potential_top_left, &self.landed_blocks);
                    if !(collides_with_element || collides_with_floor) {
                        self.current_position = potential_top_left;
                        self.current_block = potential_block;
                    } 
                } else if collides_with_right_wall {
                    let potential_top_left = BlockPosition::new(
                        potential_top_left.row, potential_top_left.column - potential_block_shape.wall_kick_distance
                    );
                    let collides_with_element = collides_with_element(potential_block, potential_top_left, &self.landed_blocks);
                    let collides_with_floor = collides_with_floor(potential_block, potential_top_left, &self.landed_blocks);
                    if !(collides_with_element || collides_with_floor) {
                        self.current_position = potential_top_left;
                        self.current_block = potential_block;
                    } 
                } else {
                    let collides_with_element = collides_with_element(potential_block, potential_top_left, &self.landed_blocks);
                    let collides_with_floor = collides_with_floor(potential_block, potential_top_left, &self.landed_blocks);
                    if !(collides_with_element || collides_with_floor) {
                        self.current_block = potential_block;
                    } 
                }
            }
        }
    }

    pub fn collapse_empty_rows(&mut self) {
        for row in 0..self.landed_blocks.rows() {
            if self.landed_blocks.landed[row].is_empty() {
                for above_row in 0..row {
                    self.landed_blocks.landed[row - above_row] = self.landed_blocks.landed[row - above_row - 1];
                }
                self.landed_blocks.landed[0].clear();
            }
        }
    }
    
    pub fn update_landed(&mut self) {
        let block = self.current_block;
        let position = self.current_position;
        self.landed_blocks.insert_block(position.row, position.column, block);
    }

    pub fn update_new_block(&mut self, block: GooglyBlock) {
        self.current_block = block;
        self.current_position = self.starting_positions[&block.piece];
    }

    pub fn collides_with_element_below(&self) -> bool {
        let shape = self.current_block.shape();
        let top_left = self.current_position;
        let landed = &self.landed_blocks;
        for (row, column) in shape.iter() {
            let element_row = row as isize;
            let element_column = column as isize;
            match landed.get(top_left.row + element_row + 1, top_left.column + element_column) {
                LandedBlocksQuery::InOfBounds(GooglyBlockElement::EmptySpace) => {}
                LandedBlocksQuery::OutOfBounds(_, _) => {}
                LandedBlocksQuery::InOfBounds(_) => return true,
            }
        }
        
        false
    }
    
    pub fn collides_with_floor_below(&self) -> bool {
        let shape = self.current_block.shape();
        let top_left = self.current_position;
        for (row, _) in shape.iter() {
            let part_row = row as isize;
            if top_left.row + part_row + 1 >= self.landed_blocks.rows() as isize {
                return true;
            }
        }
    
        false
    }
    
    pub fn collides_with_element_to_the_left(&self) -> bool {
        false
    }
    
    pub fn collides_with_left_wall(&self) -> bool {
        let shape = self.current_block.shape();
        let top_left = self.current_position;
        for (_, column) in shape.iter() {
            let element_column = column as isize;
            if top_left.column + element_column - 1 < 0 {
                return true;
            }
        }
    
        false
    }
    
    pub fn collides_with_element_to_the_right(&self) -> bool {
        false
    }
    
    pub fn collides_with_right_wall(&self) -> bool {
        let shape = self.current_block.shape();
        let top_left = self.current_position;
        let landed = &self.landed_blocks;
        for (_, column) in shape.iter() {
            let element_column = column as isize;
            if top_left.column + element_column >= landed.columns() as isize {
                return true;
            }
        }
    
        false
    }
}


#[cfg(test)]
mod landed_blocks_tests {
    use crate::block::{
        GooglyBlock, 
        GooglyBlockPiece, 
        GooglyBlockElement, 
        GooglyBlockRotation, 
    };
    use super::{
        LandedBlocksGrid, 
        LandedBlocksQuery
    };

    fn elements() -> [GooglyBlockElement; 8] { 
        use self::GooglyBlockElement::*;
        [T, J, Z, O, S, L, I, EmptySpace]
    }
    
    #[test]
    fn inserting_an_element_and_getting_it_back_yields_the_same_element() {
        let mut landed = LandedBlocksGrid::new();
        for element in elements().iter() {
            for (row, column) in landed.iter() {
                landed.insert(row, column, *element);
                let expected = LandedBlocksQuery::InOfBounds(*element);
                let result = landed.get(row, column);
        
                assert_eq!(expected, result);
            }
        }
    }

    #[test]
    fn inserting_the_same_element_to_the_same_position_twice_is_the_same_as_inserting_it_once() {
        let mut landed = LandedBlocksGrid::new();
        for element in elements().iter() {
            for (row, column) in landed.iter() {
                landed.insert(row, column, *element);
                let expected = landed.get(row, column);
                landed.insert(row, column, *element);
                let result = landed.get(row, column);
    
                assert_eq!(result, expected);
            }
        }
    }

    #[test]
    fn all_cells_in_a_new_landed_blocks_matrix_should_be_empty_spaces() {
        let landed = LandedBlocksGrid::new();
        let expected = LandedBlocksQuery::InOfBounds(GooglyBlockElement::EmptySpace);
        for (row, column) in landed.iter() {
            let result = landed.get(row, column);
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn inserting_a_block_into_landed_blocks_and_getting_it_back_yields_the_same_elements() {
        let block = GooglyBlock::new(GooglyBlockPiece::J, GooglyBlockRotation::R0);
        let shape = block.shape();
        let mut landed = LandedBlocksGrid::new();
        let top_left_row = 5;
        let top_left_column = 6;
        landed.insert_block(top_left_row, top_left_column, block);
        
        let expected = LandedBlocksQuery::InOfBounds(GooglyBlockElement::J);
        for (row, column) in shape.iter().map(|(r, c)| (r as isize, c as isize)) {
            let result = landed.get(top_left_row + row, top_left_column + column);
            assert_eq!(result, expected, "{}", landed);
        }
    }

    #[test]
    fn getting_an_element_from_a_negative_valued_row_should_be_out_of_bounds() {
        let landed = LandedBlocksGrid::new();
        assert!(landed.get(-1, 1).is_out_of_bounds());
    }

    #[test]
    fn getting_an_element_from_a_negative_valued_column_should_be_out_of_bounds() {
        let landed = LandedBlocksGrid::new();
        assert!(landed.get(1, -1).is_out_of_bounds());
    }

    #[test]
    fn getting_an_element_from_a_row_larger_than_the_number_of_rows_should_be_out_of_bounds() {
        let landed = LandedBlocksGrid::new();
        assert!(landed.get(20, 1).is_out_of_bounds());
    }

    #[test]
    fn getting_an_element_from_a_column_larger_than_the_number_of_columns_should_be_out_of_bounds() {
        let landed = LandedBlocksGrid::new();
        assert!(landed.get(1, 10).is_out_of_bounds());
    }
}

#[cfg(test)]
mod collision_tests {
    use crate::block::{
        GooglyBlock, 
        GooglyBlockPiece, 
        GooglyBlockElement,
        GooglyBlockRotation, 
    };
    use super::{
        LandedBlocksGrid,
        BlockPosition,
    };

    struct CollisionDetectionTestCase {
        landed: LandedBlocksGrid,
        occupied_cells: Vec<(isize, isize)>,
    }

    fn test_case() -> CollisionDetectionTestCase {
        let mut landed = LandedBlocksGrid::new();
        landed.insert(19, 8, GooglyBlockElement::J);
        landed.insert(19, 9, GooglyBlockElement::J);
        landed.insert(18, 9, GooglyBlockElement::J);
        landed.insert(17, 9, GooglyBlockElement::J);
        landed.insert(16, 6, GooglyBlockElement::I);
        landed.insert(17, 6, GooglyBlockElement::I);
        landed.insert(18, 6, GooglyBlockElement::I);
        landed.insert(19, 6, GooglyBlockElement::I);
        landed.insert(15, 4, GooglyBlockElement::O);
        landed.insert(15, 5, GooglyBlockElement::O);
        landed.insert(16, 4, GooglyBlockElement::O);
        landed.insert(16, 5, GooglyBlockElement::O);
        landed.insert(17, 4, GooglyBlockElement::S);
        landed.insert(18, 4, GooglyBlockElement::S);
        landed.insert(18, 5, GooglyBlockElement::S);
        landed.insert(19, 5, GooglyBlockElement::S);
        landed.insert(17, 3, GooglyBlockElement::L);
        landed.insert(16, 3, GooglyBlockElement::L);
        landed.insert(15, 3, GooglyBlockElement::L);
        landed.insert(15, 2, GooglyBlockElement::L);
        landed.insert(18, 2, GooglyBlockElement::Z);
        landed.insert(18, 3, GooglyBlockElement::Z);
        landed.insert(19, 3, GooglyBlockElement::Z);
        landed.insert(19, 4, GooglyBlockElement::Z);
        landed.insert(15, 1, GooglyBlockElement::J);
        landed.insert(15, 0, GooglyBlockElement::J);
        landed.insert(16, 0, GooglyBlockElement::J);
        landed.insert(17, 0, GooglyBlockElement::J);
        landed.insert(16, 1, GooglyBlockElement::O);
        landed.insert(16, 2, GooglyBlockElement::O);
        landed.insert(17, 1, GooglyBlockElement::O);
        landed.insert(17, 2, GooglyBlockElement::O);
        landed.insert(18, 1, GooglyBlockElement::T);
        landed.insert(19, 0, GooglyBlockElement::T);
        landed.insert(19, 1, GooglyBlockElement::T);
        landed.insert(19, 2, GooglyBlockElement::T);

        let occupied_cells = landed.iter()
            .filter(|(row, column)| landed.get(*row, *column).is_in_of_bounds())
            .filter(|(row, column)| !landed.get(*row, *column).is_empty_space())
            .collect::<Vec<(isize, isize)>>();

        CollisionDetectionTestCase {
            landed: landed,
            occupied_cells: occupied_cells,
        }
    }

    fn failed(piece: GooglyBlock, top_left: BlockPosition, landed: &LandedBlocksGrid) -> String {
        let mut new_landed = (*landed).clone();
        new_landed.insert_block(top_left.row, top_left.column, piece);
        format!("{}", new_landed)
    }

    #[test]
    fn block_elements_should_not_collide_with_unoccupied_cells() {
        let empty_landed = LandedBlocksGrid::new();
        let piece = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
        for (row, column) in empty_landed.iter() {
            assert!(!super::collides_with_element(
                piece, BlockPosition::new(row, column), &empty_landed)
            );
        }
    }

    #[test]
    fn blocks_should_collide_on_occupied_cells() {
        let test = test_case();
        let piece = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
        for (oc_row, oc_column) in test.occupied_cells.iter() {
            let top_left = BlockPosition::new(*oc_row, *oc_column);
            assert!(super::collides_with_element(piece, top_left, &test.landed));
        }
    }

    #[test]
    fn blocks_crossing_leftmost_column_should_collide_with_left_wall() {
        let landed = LandedBlocksGrid::new();
        let piece = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
        for row in (0..landed.rows()).map(|r| r as isize) {
            let top_left = BlockPosition::new(row, -1);
            assert!(super::collides_with_left_wall(piece, top_left, &landed));
        }
    }
    
    #[test]
    fn blocks_with_elements_in_leftmost_column_should_not_collide_with_left_wall() {
        let landed = LandedBlocksGrid::new();
        let piece = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
        for row in (0..landed.rows()).map(|r| r as isize) {
            let top_left = BlockPosition::new(row, 0);
            assert!(!super::collides_with_left_wall(piece, top_left, &landed), 
                "row: {}; column: {}", top_left.row, top_left.column
            );
        }        
    }

    #[test]
    fn blocks_crossing_rightmost_column_should_collide_with_right_wall() {
        let landed = LandedBlocksGrid::new();
        let piece = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
        for row in (0..landed.rows()).map(|r| r as isize) {
            let last_column = landed.columns() as isize - 1;
            let top_left = BlockPosition::new(row, last_column);
            assert!(super::collides_with_right_wall(piece, top_left, &landed),
                "row: {}; column: {}", top_left.row, top_left.column
            );
        }
    }

    #[test]
    fn blocks_with_elements_in_rightmost_column_should_not_collide_with_right_wall() {
        let landed = LandedBlocksGrid::new();
        let piece = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
        for row in (0..landed.rows()).map(|r| r as isize) {
            let top_left = BlockPosition::new(row, 7);
            assert!(!super::collides_with_right_wall(piece, top_left, &landed), 
                "row: {}; column: {}", top_left.row, top_left.column
            );
        }
    }

    #[test]
    fn blocks_crossing_floor_should_collide_with_floor() {
        let landed = LandedBlocksGrid::new();
        let piece = GooglyBlock::new(GooglyBlockPiece::I, GooglyBlockRotation::R0);
        for column in (0..landed.columns()).map(|c| c as isize) {
            let last_row = (landed.rows() - 1) as isize;
            let top_left = BlockPosition::new(last_row, column);
            assert!(super::collides_with_floor(piece, top_left, &landed));
        }
    }

    #[test]
    fn blocks_whose_bottom_elements_occupy_bottommost_row_should_not_collide_with_floor() {
        let landed = LandedBlocksGrid::new();
        let piece = GooglyBlock::new(GooglyBlockPiece::I, GooglyBlockRotation::R0);
        for column in (0..landed.columns()).map(|c| c as isize) {
            let row = (landed.rows() - 3) as isize;
            let top_left = BlockPosition::new(row, column);
            assert!(!super::collides_with_floor(piece, top_left, &landed), 
                "{}", failed(piece, top_left, &landed)
            );
        }
    }
}

#[cfg(test)]
mod playing_field_tests {
    use crate::block:: {
        GooglyBlockRotation,
        GooglyBlock, 
        GooglyBlockPiece, 
        GooglyBlockElement, 
    };
    use super::{
        LandedBlocksGrid,
        BlockPosition, 
        PlayingFieldContext, 
        PlayingFieldContextSpec, 
        GooglyBlockMove,
    };
    use std::collections::hash_map::HashMap;

    struct PlayingFieldTestCase {
        playing_field: PlayingFieldContext,
    }

    fn test_case() -> PlayingFieldTestCase {
        let mut landed_blocks = LandedBlocksGrid::new();
        landed_blocks.insert(19, 8, GooglyBlockElement::J);
        landed_blocks.insert(19, 9, GooglyBlockElement::J);
        landed_blocks.insert(18, 9, GooglyBlockElement::J);
        landed_blocks.insert(17, 9, GooglyBlockElement::J);
        landed_blocks.insert(16, 6, GooglyBlockElement::I);
        landed_blocks.insert(17, 6, GooglyBlockElement::I);
        landed_blocks.insert(18, 6, GooglyBlockElement::I);
        landed_blocks.insert(19, 6, GooglyBlockElement::I);
        landed_blocks.insert(15, 4, GooglyBlockElement::O);
        landed_blocks.insert(15, 5, GooglyBlockElement::O);
        landed_blocks.insert(16, 4, GooglyBlockElement::O);
        landed_blocks.insert(16, 5, GooglyBlockElement::O);
        landed_blocks.insert(17, 4, GooglyBlockElement::S);
        landed_blocks.insert(18, 4, GooglyBlockElement::S);
        landed_blocks.insert(18, 5, GooglyBlockElement::S);
        landed_blocks.insert(19, 5, GooglyBlockElement::S);
        landed_blocks.insert(17, 3, GooglyBlockElement::L);
        landed_blocks.insert(16, 3, GooglyBlockElement::L);
        landed_blocks.insert(15, 3, GooglyBlockElement::L);
        landed_blocks.insert(15, 2, GooglyBlockElement::L);
        landed_blocks.insert(18, 2, GooglyBlockElement::Z);
        landed_blocks.insert(18, 3, GooglyBlockElement::Z);
        landed_blocks.insert(19, 3, GooglyBlockElement::Z);
        landed_blocks.insert(19, 4, GooglyBlockElement::Z);
        landed_blocks.insert(15, 1, GooglyBlockElement::J);
        landed_blocks.insert(15, 0, GooglyBlockElement::J);
        landed_blocks.insert(16, 0, GooglyBlockElement::J);
        landed_blocks.insert(17, 0, GooglyBlockElement::J);
        landed_blocks.insert(16, 1, GooglyBlockElement::O);
        landed_blocks.insert(16, 2, GooglyBlockElement::O);
        landed_blocks.insert(17, 1, GooglyBlockElement::O);
        landed_blocks.insert(17, 2, GooglyBlockElement::O);
        landed_blocks.insert(18, 1, GooglyBlockElement::T);
        landed_blocks.insert(19, 0, GooglyBlockElement::T);
        landed_blocks.insert(19, 1, GooglyBlockElement::T);
        landed_blocks.insert(19, 2, GooglyBlockElement::T);

        let starting_block = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
        let starting_positions: HashMap<GooglyBlockPiece, BlockPosition> = [
            (GooglyBlockPiece::T, BlockPosition::new(-3, 4)),
            (GooglyBlockPiece::J, BlockPosition::new(-3, 4)), 
            (GooglyBlockPiece::Z, BlockPosition::new(-3, 4)),
            (GooglyBlockPiece::O, BlockPosition::new(-3, 4)), 
            (GooglyBlockPiece::S, BlockPosition::new(-3, 4)), 
            (GooglyBlockPiece::L, BlockPosition::new(-3, 4)),
            (GooglyBlockPiece::I, BlockPosition::new(-3, 3)),
        ].iter().map(|elem| *elem).collect();
        let spec = PlayingFieldContextSpec {
            starting_block: starting_block,
            starting_positions: starting_positions,
        };
        let mut playing_field = PlayingFieldContext::new(spec);
        playing_field.landed_blocks = landed_blocks;


        PlayingFieldTestCase {
            playing_field: playing_field,
        }
    }

    fn empty_playing_field_test_case() -> PlayingFieldTestCase {
        let starting_block = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
        let starting_positions: HashMap<GooglyBlockPiece, BlockPosition> = [
            (GooglyBlockPiece::T, BlockPosition::new(-3, 4)),
            (GooglyBlockPiece::J, BlockPosition::new(-3, 4)), 
            (GooglyBlockPiece::Z, BlockPosition::new(-3, 4)),
            (GooglyBlockPiece::O, BlockPosition::new(-3, 4)), 
            (GooglyBlockPiece::S, BlockPosition::new(-3, 4)), 
            (GooglyBlockPiece::L, BlockPosition::new(-3, 4)),
            (GooglyBlockPiece::I, BlockPosition::new(-3, 3)),
        ].iter().map(|elem| *elem).collect();
        let spec = PlayingFieldContextSpec {
            starting_block: starting_block,
            starting_positions: starting_positions,
        };
        let playing_field = PlayingFieldContext::new(spec);

        PlayingFieldTestCase {
            playing_field: playing_field,
        }
    }

    fn moves_collide_with_elements(playing_field: &mut PlayingFieldContext, moves: &[GooglyBlockMove]) -> bool {
        for mv in moves.iter() {
            let old_position = playing_field.current_position;
            playing_field.update_block_position(*mv);
            if playing_field.current_position == old_position {
                return true;
            }
        }

        false
    }

    fn moves_collide_with_floor(playing_field: &mut PlayingFieldContext, moves: &[GooglyBlockMove]) -> bool {
        for mv in moves.iter() {
            playing_field.update_block_position(*mv);
            let top_left = playing_field.current_position;
            let shape = playing_field.current_block.shape();
            let last_row = playing_field.landed_blocks.rows() as isize - 1;
            for element in shape.iter() {
                if top_left.row + element.0 as isize == last_row {
                    return true;
                }
            }
        }

        false
    }

    fn moves_collide_with_right_wall(playing_field: &mut PlayingFieldContext, moves: &[GooglyBlockMove]) -> bool {
        for mv in moves.iter() {
            playing_field.update_block_position(*mv);
            let top_left = playing_field.current_position;
            let shape = playing_field.current_block.shape();
            let last_column = playing_field.landed_blocks.columns() as isize - 1;
            for element in shape.iter() {
                if top_left.column + element.1 as isize == last_column {
                    return true;
                }
            }
        }

        false
    }

    fn moves_collide_with_left_wall(playing_field: &mut PlayingFieldContext, moves: &[GooglyBlockMove]) -> bool {
        for mv in moves.iter() {
            playing_field.update_block_position(*mv);
            let top_left = playing_field.current_position;
            let shape = playing_field.current_block.shape();
            let first_column = 0;
            for element in shape.iter() {
                if top_left.column + element.1 as isize == first_column {
                    return true;
                }
            }
        }

        false
    }

    #[test]
    fn falls_should_collide_with_element_in_playing_field() {
        let mut test = test_case();
        let moves = vec![GooglyBlockMove::Fall; 20];
        assert!(moves_collide_with_elements(&mut test.playing_field, &moves));
    }

    #[test]
    fn falling_in_an_empty_playing_field_should_stop_on_floor() {
        let mut test = empty_playing_field_test_case();
        let moves = vec![GooglyBlockMove::Fall; 30];
        assert!(moves_collide_with_floor(&mut test.playing_field, &moves));
    }

    #[test]
    fn moving_far_enough_right_in_an_empty_playing_field_should_stop_on_right_wall() {
        let mut test = empty_playing_field_test_case();
        let moves = vec![GooglyBlockMove::Right; 20];
        assert!(moves_collide_with_right_wall(&mut test.playing_field, &moves));
    }

    #[test]
    fn moving_far_enough_left_in_an_empty_playing_field_should_stop_on_left_wall() {
        let mut test = empty_playing_field_test_case();
        let moves = vec![GooglyBlockMove::Left; 20];
        assert!(moves_collide_with_left_wall(&mut test.playing_field, &moves));
    }
}
