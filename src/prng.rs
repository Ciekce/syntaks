pub struct Sfc64 {
    a: u64,
    b: u64,
    c: u64,
    counter: u64,
}

impl Sfc64 {
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        let mut result = Self {
            a: seed,
            b: seed,
            c: seed,
            counter: 1,
        };

        let mut i = 0;
        while i < 12 {
            result.next_u64();
            i += 1;
        }

        result
    }

    pub const fn next_u64(&mut self) -> u64 {
        let result = self.a.wrapping_add(self.b).wrapping_add(self.counter);
        self.counter = self.counter.wrapping_add(1);
        self.a = self.b ^ (self.b >> 11);
        self.b = self.c.wrapping_add(self.c << 3);
        self.c = self.c.rotate_left(24).wrapping_add(result);
        result
    }

    pub const fn fill(&mut self, values: &mut [u64]) {
        let mut idx = 0;
        while idx < values.len() {
            values[idx] = self.next_u64();
            idx += 1;
        }
    }
}

#[must_use]
pub const fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

#[must_use]
pub fn seed_from_entropy() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    splitmix64(nanos ^ (std::process::id() as u64).rotate_left(32))
}
