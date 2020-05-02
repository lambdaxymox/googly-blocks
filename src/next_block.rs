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


struct NextBlockGen {
    rng: rng::ThreadRng,
    between: Uniform<u32>,
    last_block: GooglyBlock,
    table: [GooglyBlock; 7],
}

impl NextBlockGen {
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

    fn next(&mut self) -> GooglyBlock {
        let mut block = self.table[self.between.sample(&mut self.rng) as usize];
        let mut gas = 0;
        while (gas < 8) && (block == self.last_block) {
            let random = self.between.sample(&mut self.rng) as usize;
            block = self.table[random];
            gas += 1;
        }
        self.last_block = block;
        
        block
    }
}

pub struct NextBlockCell {
    gen: NextBlockGen,
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

    pub fn update(&mut self) {
        self.block = self.gen.next();
    }

    #[inline]
    pub fn block(&self) -> GooglyBlock {
        self.block
    }
}
