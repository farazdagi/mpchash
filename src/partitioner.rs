use {
    crate::RingPosition,
    std::hash::{BuildHasher, Hash, Hasher},
    xxhash_rust::xxh3::Xxh3Builder,
};

/// A keyspace partitioning strategy.
///
/// Partitioner is responsible for mapping hashable objects to positions on the
/// ring i.e. it knows how to partition the keyspace.
pub trait Partitioner<K: Hash> {
    /// Returns ring position for a given key (using default seed).
    fn position(&self, key: &K) -> RingPosition;

    /// Returns ring position for a given key (using a given seed).
    ///
    /// By supplying a seed, we can have different positions for the same key.
    /// This is particularly useful when implementing double-hashing.
    fn position_seeded(&self, key: &K, seed: RingPosition) -> RingPosition;
}

/// Seeds for double hashing.
///
/// Essentially, we can use any seeds, to initialize the hasher (XXH3 uses `0`
/// by default).
pub const DEFAULT_SEED1: u64 = 12345;
pub const DEFAULT_SEED2: u64 = 67890;

/// A partitioner that uses a XXH3 hash function to partition data.
#[derive(Clone)]
pub struct Xxh3Partitioner {
    hash_builder: Xxh3Builder,
}

impl Default for Xxh3Partitioner {
    fn default() -> Self {
        Self {
            hash_builder: Xxh3Builder::new(),
        }
    }
}

impl Xxh3Partitioner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hash<K: Hash>(&self, key: &K, seed: RingPosition) -> RingPosition {
        let mut hasher = self.hash_builder.with_seed(seed).build_hasher();
        key.hash(&mut hasher);
        hasher.finish()
    }
}

impl<K: Hash> Partitioner<K> for Xxh3Partitioner {
    fn position(&self, key: &K) -> RingPosition {
        self.hash(key, DEFAULT_SEED1)
    }

    fn position_seeded(&self, key: &K, seed: RingPosition) -> RingPosition {
        self.hash(key, seed)
    }
}

/// Default partitioner.
pub type DefaultPartitioner = Xxh3Partitioner;
