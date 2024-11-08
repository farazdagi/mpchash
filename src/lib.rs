#![doc = include_str!("../README.md")]

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
pub const DEFAULT_PROBE_COUNT: usize = 23;

/// Position on the ring.
pub type RingPosition = u64;

/// Node that can be assigned a position on the ring.
pub trait RingNode: Hash + Clone + Debug + Eq + PartialEq + Ord + PartialOrd {}

/// Blanket implementation of `RingNode` for all types that implement the
/// necessary traits.
impl<T> RingNode for T where T: Hash + Clone + Debug + Eq + PartialEq + Ord + PartialOrd {}

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
    probe_count: usize,
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

impl<N: RingNode> Debug for HashRing<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashRing")
            .field("positions", &self.positions)
            .field("probe_count", &self.probe_count)
            .finish_non_exhaustive()
    }
}

impl<N: RingNode> HashRing<N> {
    /// Creates a new hash ring.
    ///
    /// Any type implementing [`RingNode`] can be used as a node type.
    ///
    /// # Examples
    ///
    /// Create ring with `u64` nodes:
    /// ```
    /// let mut ring = mpchash::HashRing::<u64>::new();
    /// ring.add(0);
    /// ring.add(2)
    /// ```
    ///
    /// Create ring with custom node type:
    /// ```
    /// use mpchash::{HashRing, RingNode};
    ///
    /// #[derive(Hash, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
    /// struct Node {
    ///     id: u64,
    /// }
    ///
    /// let mut ring = HashRing::<Node>::new();
    /// ring.add(Node { id: 0 });
    /// ring.add(Node { id: 2 });
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new node to the ring.
    ///
    /// The position is computed deterministically using keyspace partitioner.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut ring = mpchash::HashRing::<u64>::new();
    /// ring.add(0);
    /// ```
    pub fn add(&mut self, node: N) {
        let pos = self.partitioner.position(&node);
        self.positions.insert(pos, node);
    }

    /// Inserts a node to a given ring position.
    ///
    /// Mostly useful for testing and simulation, use `add` in all other cases.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut ring = mpchash::HashRing::<u64>::new();
    /// // Insert node "15" at position 0.
    /// ring.insert(0, 15);
    /// // Insert node "16" at position 1.
    /// ring.insert(1, 16);
    /// ```
    pub fn insert(&mut self, pos: RingPosition, node: N) {
        self.positions.insert(pos, node);
    }

    /// Removes a node from the ring.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut ring = mpchash::HashRing::<u64>::new();
    /// ring.add(42);
    /// ring.remove(&42);
    /// ```
    pub fn remove(&mut self, node: &N) {
        let pos = self.partitioner.position(node);
        self.positions.remove(&pos);
    }

    /// Returns the primary node responsible for the given key.
    ///
    /// Due to replication, a key may land on several nodes, but the primary
    /// destination is the node controlling ring position coming immediately
    /// after the key.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut ring = mpchash::HashRing::<u64>::new();
    /// for i in 0..6 {
    ///     ring.add(i);
    /// }
    /// for i in 0..100 {
    ///     println!(
    ///         "key {i} should go to node {}",
    ///         ring.primary_node(&i).expect("no node found for key")
    ///     );
    /// }
    /// ```
    pub fn primary_node<K: Hash>(&self, key: &K) -> Option<&N> {
        self.primary_token(key).map(|token| token.1)
    }

    /// Returns the token of a node that owns a range for the given key.
    ///
    /// A token is a pair of a ring position of a node and a node itself.
    ///
    /// In replicated setting a single range is owned by multiple nodes (which
    /// are basically the first `n` nodes when moving clockwise from the
    /// selected probe), but the first node is considered as primary.
    ///
    /// Double hashing is used to avoid non-uniform distribution of keys across
    /// the ring. From the multiple produced positions, the one with the
    /// minimal distance to the next node is selected.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut ring = mpchash::HashRing::<u64>::new();
    /// for i in 0..6 {
    ///     ring.add(i);
    /// }
    /// for i in 0..100 {
    ///     let (pos, node) = ring
    ///         .primary_token(&i)
    ///         .expect("no primary token found for key");
    ///     println!("key {i} should go to node {node} at position {pos}");
    /// }
    /// ```
    pub fn primary_token<K: Hash>(&self, key: &K) -> Option<RingToken<N>> {
        let mut min_distance = RingPosition::MAX;
        let mut min_token = None;

        // Calculate several positions for the given key and select the one with the
        // minimal distance to the owner.
        for pos in self.partitioner.positions(key, self.probe_count) {
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
    ///
    /// # Examples
    ///
    /// ```
    /// let mut ring = mpchash::HashRing::<u64>::new();
    /// for i in 0..6 {
    ///     ring.add(i);
    /// }
    /// for (pos, node) in ring.tokens(0, mpchash::RingDirection::Clockwise) {
    ///     println!("node {} is at position {}", node, pos);
    /// }
    /// // We can move in both directions.
    /// for (pos, node) in ring.tokens(0, mpchash::RingDirection::CounterClockwise) {
    ///     println!("node {} is at position {}", node, pos);
    /// }
    /// ```
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

    /// Returns the key space range owned by a node, if it was located at given
    /// position.
    ///
    /// If range is available, it always ends at the given position, and starts
    /// at the position to the left (counter-clockwise) of the provided `pos`.
    /// If range is not available, on an empty ring, for example, `None` is
    /// returned.
    ///
    /// Note: since we semantically treat the ordered set as a ring, the key
    /// range wraps around.
    ///
    /// # Examples
    ///
    /// ```
    /// use mpchash::{HashRing, RingPosition};
    ///
    /// let mut ring = HashRing::new();
    ///
    /// // Define nodes.
    /// let node1 = "SomeNode1";
    /// let node2 = "SomeNode2";
    ///
    /// // Add nodes to the ring.
    /// ring.add(node1);
    /// ring.add(node2);
    ///
    /// // Get the range owned by node1.
    /// let pos = ring.position(&node1);
    /// let range = ring.key_range(pos).unwrap();
    ///
    /// // The range starts at the position to the left of node1,
    /// // till (and not including) its own position.
    /// assert_eq!(range.start, ring.position(&node2));
    /// assert_eq!(range.end, ring.position(&node1));
    /// ```
    pub fn key_range(&self, pos: RingPosition) -> Option<KeyRange<RingPosition>> {
        if self.positions.is_empty() {
            return None;
        }
        let prev_pos = self.tokens(pos, Clockwise).next_back();
        let start = prev_pos.map_or(0, |token| *token.0);
        Some(KeyRange::new(start, pos))
    }

    /// Returns ring position to which a given key will be assigned.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut ring = mpchash::HashRing::<u64>::new();
    /// let key = "some key";
    /// // Find the position of the key on the ring.
    /// let pos = ring.position(&key);
    /// ```
    pub fn position<K: Hash>(&self, key: &K) -> RingPosition {
        self.partitioner.position(key)
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
