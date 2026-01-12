use crate::takmove::Move;

pub const DEFAULT_TT_SIZE_MIB: usize = 64;
pub const MAX_TT_SIZE_MIB: usize = 131072;

#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
pub struct Entry {
    pub key: u16,
    pub mv: Option<Move>,
}

#[must_use]
fn calc_entry_count(size_mib: usize) -> usize {
    size_mib * 1024 * 1024 / size_of::<Entry>()
}

#[must_use]
fn pack_entry_key(key: u64) -> u16 {
    key as u16
}

pub struct TranspositionTable {
    entries: Vec<Entry>,
}

impl TranspositionTable {
    pub fn new(size_mib: usize) -> TranspositionTable {
        assert!(size_mib > 0);

        let mut result = Self {
            entries: Vec::default(),
        };

        result.resize(size_mib);

        result
    }

    pub fn resize(&mut self, size_mib: usize) {
        self.entries.clear();
        self.entries.shrink_to_fit();

        let entry_count = calc_entry_count(size_mib);
        self.entries.resize(entry_count, Default::default());

        self.clear();
    }

    pub fn probe(&self, key: u64) -> Option<Entry> {
        let idx = self.calc_index(key);
        let entry_key = pack_entry_key(key);

        //SAFETY: index() cannot return an out-of-bounds index
        let entry = *unsafe { self.entries.get_unchecked(idx) };

        if entry.key != entry_key {
            return None;
        }

        if entry.mv.is_some() {
            return Some(entry);
        }

        None
    }

    pub fn store(&mut self, key: u64, mv: Move) {
        let idx = self.calc_index(key);
        let entry_key = pack_entry_key(key);

        let entry = Entry {
            key: entry_key,
            mv: Some(mv),
        };

        //SAFETY: index() cannot return an out-of-bounds index
        *unsafe { self.entries.get_unchecked_mut(idx) } = entry;
    }

    pub fn clear(&mut self) {
        self.entries.fill(Default::default());
    }

    #[must_use]
    fn calc_index(&self, key: u64) -> usize {
        ((key as u128 * self.entries.len() as u128) >> 64) as usize
    }
}
