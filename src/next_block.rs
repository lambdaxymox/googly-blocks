use rand::prelude as rng;
use rand::distributions::{
    Distribution, 
    Uniform
};
use playing_field::{
    GooglyBlockPiece,
    GooglyBlockRotation,
    GooglyBlock,   
};


/// The generator than pseudorandomly generates the next block for the 
/// next block panel in the game.
struct NextBlockGen {
    /// The inner random number generator.
    rng: rng::ThreadRng,
    /// The probability distribution for choosing the next block.
    between: Uniform<u32>,
    /// The last block generated.
    last_block: GooglyBlock,
    /// The table of possible blocks that can be generated.
    table: [GooglyBlock; 7],
}

impl NextBlockGen {
    /// Construct a new block generator. Here, we choose each block to have the default rotation 
    /// state of R0 since that is what the panel displays.
    fn new() -> NextBlockGen {
        let table = [
            GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0),
            GooglyBlock::new(GooglyBlockPiece::J, GooglyBlockRotation::R0),
            GooglyBlock::new(GooglyBlockPiece::Z, GooglyBlockRotation::R0),
            GooglyBlock::new(GooglyBlockPiece::O, GooglyBlockRotation::R0),
            GooglyBlock::new(GooglyBlockPiece::S, GooglyBlockRotation::R0),
            GooglyBlock::new(GooglyBlockPiece::L, GooglyBlockRotation::R0),
            GooglyBlock::new(GooglyBlockPiece::I, GooglyBlockRotation::R0),
        ];
        let mut rng = rng::thread_rng();
        let between = Uniform::new_inclusive(0, 6);
        let random = between.sample(&mut rng) as usize;
        let last_block = table[random];

        NextBlockGen {
            rng: rng,
            between: between,
            last_block: last_block,
            table: table,
        }
    }

    /// Generate the next block.
    fn next(&mut self) -> GooglyBlock {
        let mut block = self.table[self.between.sample(&mut self.rng) as usize];
        let mut gas = 0;
        // We perform a bounded iteration over the random number generator
        // to reduce the probability of generating long runs of the same pieces.
        // The gas parameter exists to guarantee that the loop terminates.
        while (gas < 8) && (block == self.last_block) {
            let random = self.between.sample(&mut self.rng) as usize;
            block = self.table[random];
            gas += 1;
        }
        self.last_block = block;
        
        block
    }
}

/// The next block cell holds the next block in the window, which is also the 
/// next block that will be generated for the player.
pub struct NextBlockCell {
    /// The inner block generator.
    gen: NextBlockGen,
    /// The current block given to the player.
    block: GooglyBlock,
}

impl NextBlockCell {
    pub fn new() -> NextBlockCell {
        let mut gen = NextBlockGen::new();
        let block = gen.next();
        
        NextBlockCell {
            gen: gen,
            block: block,
        }
    }

    /// Generate the next block.
    pub fn update(&mut self) {
        self.block = self.gen.next();
    }

    /// Get the current block.
    #[inline]
    pub fn current_block(&self) -> GooglyBlock {
        self.block
    }
}
