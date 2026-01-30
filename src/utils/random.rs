use rand::rngs::StdRng;

use crate::prelude::*;

pub fn mix_u128_complex(seed: u128) -> u64 {
    let mut low = seed as u64;
    let mut high = (seed >> 64) as u64;

    low = low.wrapping_mul(0x4cf5ad432745937f) ^ high;
    high = high.wrapping_mul(0x1b873593) ^ low;

    low = low.rotate_left(17) ^ high.rotate_right(29);
    high = high.rotate_left(31) ^ low.rotate_right(19);

    low = low.wrapping_mul(0xbf58476d1ce4e5b9);
    high = high.wrapping_mul(0x94d049bb133111eb);

    low ^ high
}

pub fn gen_rnd() -> Result<StdRng> {
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use std::time::{SystemTime, UNIX_EPOCH};

    let random_seed = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
    Ok(StdRng::seed_from_u64(mix_u128_complex(random_seed)))
}
