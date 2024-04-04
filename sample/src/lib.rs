#![allow(clippy::disallowed_macros)]

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

    // Will appear in --llvm-input (before LLVM passes), but not in --llvm (after LLVM passes).
    #[inline]
    fn from_seed(seed: Self::Seed) -> Self {
        Self(seed)
    }
}

#[cfg(not(feature = "superbanana"))]
#[inline(never)]
pub fn main() -> u32 {
    1 + 1
}

#[inline(never)]
pub fn panics() {
    panic!("oh noes asdf wef wef wf wefwefwef wef! {}", "bob");
}

pub struct Bar(pub u32);
impl Bar {
    #[no_mangle]
    pub fn make_bar(a: u32, b: u32) -> Self {
        Self(a + b)
    }
}

#[cfg(feature = "superbanana")]
#[inline(never)]
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
        println!("{x}");
    }

    println!("Total: {}", get_length(set));
}

#[inline(never)]
fn get_length<T>(it: hashbrown::HashSet<T>) -> usize {
    it.len()
}

pub fn okay() {
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
        println!("{x}");
    }
}
