use rand_core::block::{BlockRng, BlockRngCore};
use rand_core::{RngCore, SeedableRng};

pub struct MyRngCore(pub [u8; 32]);

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

pub fn main() {
    let mut rng = BlockRng::<MyRngCore>::seed_from_u64(0);
    for ix in 0..10 {
        println!("{ix} rng values: {}", rng.next_u32());
    }

    use hashbrown::HashSet;
    let mut set = HashSet::new();
    set.insert("a");
    set.insert("b");

    // Will print in an arbitrary order.
    for x in set.iter() {
        println!("{}", x);
    }
}
