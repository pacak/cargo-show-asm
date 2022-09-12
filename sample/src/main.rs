use rand_core::block::{BlockRng, BlockRngCore};
use rand_core::{RngCore, SeedableRng};

struct MyRngCore([u8; 32]);

impl BlockRngCore for MyRngCore {
    type Item = u32;
    type Results = [u32; 16];

    fn generate(&mut self, results: &mut Self::Results) {
        for (to, from) in std::iter::zip(results.iter_mut(), self.0.iter()) {
            *to = *from as u32;
        }
    }
}

impl SeedableRng for MyRngCore {
    type Seed = [u8; 32];
    fn from_seed(seed: Self::Seed) -> Self {
        Self(seed)
    }
}

fn main() {
    let mut rng = BlockRng::<MyRngCore>::seed_from_u64(0);
    println!("First value: {}", rng.next_u32());
}

