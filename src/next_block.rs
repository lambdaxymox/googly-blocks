use rand::prelude as rng;
use rand::distributions::{
    Distribution, 
    Uniform
};
use playing_field::GooglyBlockPiece;


struct NextBlockGen {
    rng: rng::ThreadRng,
    between: Uniform<u32>,
    last_block: GooglyBlockPiece,
    table: [GooglyBlockPiece; 7],
}

impl NextBlockGen {
    fn new() -> NextBlockGen {
        let table = [
            GooglyBlockPiece::T,
            GooglyBlockPiece::J,
            GooglyBlockPiece::Z,
            GooglyBlockPiece::O,
            GooglyBlockPiece::S,
            GooglyBlockPiece::L,
            GooglyBlockPiece::I,
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

    fn next(&mut self) -> GooglyBlockPiece {
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
    block: GooglyBlockPiece,
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
    pub fn block(&self) -> GooglyBlockPiece {
        self.block
    }
}
