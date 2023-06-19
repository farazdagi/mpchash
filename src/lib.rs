mod iter;
mod partitioner;
mod range;

use {
    crate::{
        iter::HashRingIter,
        RingDirection::{Clockwise, CounterClockwise},
    },
    std::{
        collections::BTreeMap,
        fmt::Debug,
        hash::Hash,
        ops::Bound::{Excluded, Unbounded},
    },
};
pub use {partitioner::*, range::*};

/// Number of probing attempts before selecting key's position on the ring.
///
/// The probe with minimal distance to some assigned node is selected. Then the
/// first node when moving clockwise from the selected probe is deemed to be key
/// owner.
pub const DEFAULT_PROBE_COUNT: u16 = 23;

/// Position on the ring.
pub type RingPosition = u64;

/// Node that can be assigned a position on the ring.
pub trait RingNode: Hash + Clone + Copy + Debug + Eq + PartialEq + Ord + PartialOrd {}

/// An ownership over a position on the ring (by the object of type `T`,
/// normally, `RingNode`).
pub type RingToken<'a, T> = (&'a RingPosition, &'a T);

/// Defines the direction in which the ring is traversed.
#[derive(Clone, Copy)]
pub enum RingDirection {
    Clockwise,
    CounterClockwise,
}

/// Consistent hash ring.
///
/// Nodes are assigned positions on the ring, effectively becoming responsible
/// for a range of keys: from the previous node (counter-clockwise) up to and
/// not including the node's position.
#[derive(Clone)]
pub struct HashRing<N: RingNode, P = DefaultPartitioner> {
    /// Partitioner used to compute ring positions.
    partitioner: P,

    /// The ring positions assigned to nodes (sorted in ascending order).
    positions: BTreeMap<RingPosition, N>,

    /// The number of positions to probe for a given key.
    probe_count: u16,
}

impl<N: RingNode> Default for HashRing<N> {
    fn default() -> Self {
        Self {
            partitioner: DefaultPartitioner::new(),
            positions: BTreeMap::new(),
            probe_count: DEFAULT_PROBE_COUNT,
        }
    }
}

impl<N: RingNode> HashRing<N> {
    /// Creates a new hash ring.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new node to the ring.
    ///
    /// The position is computed deterministically using keyspace partitioner.
    pub fn add(&mut self, node: N) {
        let pos = self.partitioner.position(&node);
        self.positions.insert(pos, node);
    }

    /// Inserts a node to a given ring position.
    ///
    /// Mostly useful for testing and simulation, use `add` in all other cases.
    pub fn insert(&mut self, pos: RingPosition, node: N) {
        self.positions.insert(pos, node);
    }

    /// Removes a node from the ring.
    pub fn remove(&mut self, node: &N) {
        let pos = self.partitioner.position(node);
        self.positions.remove(&pos);
    }

    /// Returns the primary node responsible for the given key.
    ///
    /// Due to replication, a key may land on several nodes, but the primary
    /// destination is the node controlling ring position coming immediately
    /// after the key.
    pub fn primary_node<K: Hash>(&self, key: &K) -> Option<&N> {
        self.primary_token(key).map(|token| token.1)
    }

    /// Returns the token of a node that owns a range for the given key.
    ///
    /// In replicated setting a single range is owned by multiple nodes (which
    /// are basically the first `n` nodes when moving clockwise from the
    /// selected probe), but the first node is considered as primary.
    ///
    /// Double hashing is used to avoid non-uniform distribution of keys across
    /// the ring. From the multiple produced positions, the one with the
    /// minimal distance to the next node is selected.
    pub fn primary_token<K: Hash>(&self, key: &K) -> Option<RingToken<N>> {
        let mut min_distance = RingPosition::MAX;
        let mut min_token = None;
        let h1 = self.partitioner.position_seeded(key, DEFAULT_SEED1);
        let h2 = self.partitioner.position_seeded(key, DEFAULT_SEED2);

        // Calculate several positions for the given key and select the one with the
        // minimal distance to the owner.
        for i in 0..self.probe_count {
            // pos = h1 + i * h2
            let pos = h1.wrapping_add((i as RingPosition).wrapping_mul(h2));

            // Find the peer that owns the position, and calculate the distance to it.
            match self.tokens(pos, Clockwise).next() {
                Some((next_pos, next_peer_id)) => {
                    let distance = distance(pos, *next_pos);
                    if distance < min_distance {
                        min_distance = distance;
                        min_token = Some((next_pos, next_peer_id));
                    }
                }
                None => {
                    return None;
                }
            };
        }

        min_token
    }

    /// Returns assigned node positions (tokens) starting from the given
    /// location on the ring.
    ///
    /// One can go in both directions, clockwise and counter-clockwise, allowing
    /// to see both the next assigned positions and the previous ones. Since we
    /// position nodes on a ring, when maximum position is reached, the next
    /// position is the minimum one (positions wrap around). Hence, we chain
    /// another iterator, to account for this semantics.
    #[must_use]
    pub fn tokens(
        &self,
        start: RingPosition,
        dir: RingDirection,
    ) -> impl DoubleEndedIterator<Item = RingToken<N>> {
        match dir {
            Clockwise => HashRingIter::Clockwise(
                self.positions
                    .range(start..)
                    .chain(self.positions.range(0..start)),
            ),
            CounterClockwise => HashRingIter::CounterClockwise(
                self.positions
                    .range(..=start)
                    .rev()
                    // We must exclude start position i.e. `(start..)`.
                    .chain(self.positions.range((Excluded(start), Unbounded)).rev()),
            ),
        }
    }

    /// Returns size of the ring, i.e. number of contained tokens.
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// Returns `true` if the ring is empty.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

/// Calculates distance between two ring positions.
fn distance(pos1: RingPosition, pos2: RingPosition) -> RingPosition {
    if pos1 > pos2 {
        RingPosition::MAX - pos1 + pos2
    } else {
        pos2 - pos1
    }
}
