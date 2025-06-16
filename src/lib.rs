#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod iter;
mod partitioner;
mod range;
mod token;

use {
    crate::{
        iter::HashRingIter,
        RingDirection::{Clockwise, CounterClockwise},
    },
    crossbeam_skiplist::SkipMap,
    std::{
        hash::Hash,
        ops::Bound::{Excluded, Unbounded},
        sync::Arc,
    },
};
pub use {partitioner::*, range::*, token::RingToken};

/// Node that serves as a destination for data.
///
/// Node controls one or more interval of the key space.
/// Keys which fall into such an interval are routed to the node.
pub trait RingNode: Hash + Send + 'static {}

impl<T> RingNode for T where T: Hash + Send + 'static {}

/// Number of probing attempts before selecting key's position on the ring.
///
/// The probe with minimal distance to some assigned node is selected. Then the
/// first node when moving clockwise from the selected probe is deemed to be key
/// owner.
pub const DEFAULT_PROBE_COUNT: usize = 23;

/// Position on the ring.
pub type RingPosition = u64;

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
    positions: Arc<SkipMap<RingPosition, N>>,

    /// The number of positions to probe for a given key.
    probe_count: usize,
}

impl<N: RingNode> Default for HashRing<N> {
    fn default() -> Self {
        Self {
            partitioner: DefaultPartitioner::new(),
            positions: Arc::new(SkipMap::new()),
            probe_count: DEFAULT_PROBE_COUNT,
        }
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
    /// let ring = mpchash::HashRing::<u64>::new();
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
    /// let ring = HashRing::<Node>::new();
    /// ring.add(Node { id: 0 });
    /// ring.add(Node { id: 2 });
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a node to a given ring position.
    ///
    /// Mostly useful for testing and simulation, use `add` in all other cases.
    ///
    /// # Examples
    ///
    /// ```
    /// let ring = mpchash::HashRing::<u64>::new();
    /// // Insert node "15" at position 0.
    /// ring.insert(0, 15);
    /// // Insert node "16" at position 1.
    /// ring.insert(1, 16);
    /// ```
    pub fn insert(&self, pos: RingPosition, node: N) {
        self.positions.insert(pos, node);
    }

    /// Adds a new node to the ring.
    ///
    /// The position is computed deterministically using keyspace partitioner.
    pub fn add(&self, node: N) {
        let pos = self.partitioner.position(&node);
        self.positions.insert(pos, node);
    }

    /// Removes a node from the ring.
    ///
    /// # Examples
    ///
    /// ```
    /// let ring = mpchash::HashRing::<u64>::new();
    /// ring.add(42);
    /// ring.remove(&42);
    /// ```
    pub fn remove(&self, node: &N) {
        let pos = self.partitioner.position(node);
        self.positions.remove(&pos);
    }

    /// Returns `k` nodes responsible for the given key.
    ///
    /// The first node is the primary node responsible for the key. It is
    /// guaranteed that the first node is the same as the one returned by
    /// [`node()`](Self::node).
    pub fn replicas<K: Hash>(&self, key: &K, k: usize) -> Vec<RingToken<'_, N>> {
        self.tokens(self.position(key), Clockwise)
            .take(k)
            .collect::<Vec<_>>()
    }

    /// Returns intervals of the key space controlled by the given node.
    ///
    /// This method is necessary to re-balance the key space. When a node is
    /// added or removed, data needs to be moved from one node to another.
    /// In order to do so, the current intervals controlled by the node need
    /// to be known.
    ///
    /// Whenever the node is not part of the key space, `None` is returned.
    pub fn intervals(&self, node: &N) -> Option<Vec<KeyRange<RingPosition>>> {
        let pos = self.position(node);
        self.key_range(pos).map(|range| vec![range])
    }

    /// Returns ring position to which a given key will be assigned.
    ///
    /// # Examples
    ///
    /// ```
    /// let ring = mpchash::HashRing::<u64>::new();
    /// let key = "some key";
    /// // Find the position of the key on the ring.
    /// let pos = ring.position(&key);
    /// ```
    pub fn position<K: Hash>(&self, key: &K) -> RingPosition {
        self.partitioner.position(key)
    }

    /// Returns the primary node responsible for the given key.
    ///
    /// Due to replication, a key may land on several nodes, but the primary
    /// destination is the node controlling ring position coming immediately
    /// after the key.
    pub fn node<K: Hash>(&self, key: &K) -> Option<RingToken<N>> {
        self.primary_token(key)
    }

    /// Returns a list of all nodes currently in the ring.
    pub fn nodes(&self) -> Vec<RingToken<N>> {
        self.positions.iter().map(Into::into).collect()
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
    fn primary_token<K: Hash>(&self, key: &K) -> Option<RingToken<N>> {
        let mut min_distance = RingPosition::MAX;
        let mut min_token = None;

        // Calculate several positions for the given key and select the one with the
        // minimal distance to the owner.
        for pos in self.partitioner.positions(key, self.probe_count) {
            // Find the peer that owns the position, and calculate the distance to it.
            match self.tokens(pos, Clockwise).next() {
                Some(token) => {
                    let distance = distance(pos, token.position());
                    if distance < min_distance {
                        min_distance = distance;
                        min_token = Some(token);
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
    fn tokens(
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
        .map(Into::into)
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
    /// let ring = HashRing::new();
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
        let start = prev_pos.map_or(0, |token| token.position());
        Some(KeyRange::new(start, pos))
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
const fn distance(pos1: RingPosition, pos2: RingPosition) -> RingPosition {
    if pos1 > pos2 {
        RingPosition::MAX - pos1 + pos2
    } else {
        pos2 - pos1
    }
}

#[cfg(test)]
mod tests {
    use {super::*, rand::random, std::collections::BTreeSet};

    #[derive(Hash, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct Node {
        id: u64,
    }

    impl Node {
        fn random() -> Self {
            Self { id: random() }
        }
    }

    #[test]
    fn tokens() {
        let ring = HashRing::new();
        let node1 = Node::random();
        let node2 = Node::random();
        let node3 = Node::random();
        ring.add(node1);
        ring.add(node2);
        ring.add(node3);

        // Traverse from the beginning (clockwise).
        let positions = ring
            .tokens(0, Clockwise)
            .map(|token| *token.node())
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 3);
        assert!(positions.contains(&node1));
        assert!(positions.contains(&node2));
        assert!(positions.contains(&node3));

        // Traverse from the beginning (counter-clockwise).
        let positions = ring
            .tokens(0, CounterClockwise)
            .map(|token| *token.node())
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 3);
        assert!(positions.contains(&node1));
        assert!(positions.contains(&node2));
        assert!(positions.contains(&node3));

        // Unregister peer2, and make sure that it is no longer assigned to the ring.
        ring.remove(&node2);
        let positions = ring
            .tokens(0, Clockwise)
            .map(|token| *token.node())
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 2);
        assert!(positions.contains(&node1));
        assert!(!positions.contains(&node2));
        assert!(positions.contains(&node3));
    }

    #[test]
    fn tokens_wrap_around() {
        let ring = HashRing::new();
        let nodes = vec![Node::random(), Node::random(), Node::random()];
        nodes.iter().for_each(|node| ring.add(*node));

        // Start from position near the end of the ring (wrap around, clockwise).
        let positions = ring
            .tokens(u64::MAX - 1, Clockwise)
            .map(|token| *token.node())
            .collect::<Vec<_>>();
        assert_eq!(
            BTreeSet::from_iter(positions),
            BTreeSet::from_iter(nodes.clone())
        );

        // Start from position near zero of the ring (wrap around, counter-clockwise).
        let positions = ring
            .tokens(1, CounterClockwise)
            .map(|token| *token.node())
            .collect::<Vec<_>>();
        assert_eq!(BTreeSet::from_iter(positions), BTreeSet::from_iter(nodes));
    }

    #[track_caller]
    fn assert_nodes(ring: &HashRing<Node>, start: u64, dir: RingDirection, expected: Vec<Node>) {
        let positions = ring
            .tokens(start, dir)
            .map(|token| *token.node())
            .collect::<Vec<_>>();
        assert_eq!(positions, expected);
    }

    #[test]
    fn tokens_corner_cases() {
        let ring = HashRing::new();
        let node1 = Node::random();
        let node2 = Node::random();
        let node3 = Node::random();

        // Nodes at zero, max/2, and max.
        ring.insert(0, node1);
        ring.insert(u64::MAX / 2, node2);
        ring.insert(u64::MAX, node3);

        let test_cases = vec![
            // [0, 0)
            (0, Clockwise, vec![node1, node2, node3]),
            (0, CounterClockwise, vec![node1, node3, node2]),
            // [1, 1)
            (1, Clockwise, vec![node2, node3, node1]),
            (1, CounterClockwise, vec![node1, node3, node2]),
            // [max/2, max/2)
            (u64::MAX / 2, Clockwise, vec![node2, node3, node1]),
            (u64::MAX / 2, CounterClockwise, vec![node2, node1, node3]),
            // [max/2 + 1, max/2 + 1)
            (u64::MAX / 2 + 1, Clockwise, vec![node3, node1, node2]),
            (u64::MAX / 2 + 1, CounterClockwise, vec![
                node2, node1, node3,
            ]),
            // [max, max)
            (u64::MAX, Clockwise, vec![node3, node1, node2]),
            (u64::MAX, CounterClockwise, vec![node3, node2, node1]),
        ];
        for (start, dir, expected) in test_cases {
            assert_nodes(&ring, start, dir, expected);
        }
    }

    #[test]
    fn tokens_for_key() {
        let ring = HashRing::new();
        let node1 = Node::random();
        let node2 = Node::random();
        let node3 = Node::random();
        ring.add(node1);
        ring.add(node2);
        ring.add(node3);

        let tokens = ring
            .tokens(ring.position(&"foo"), Clockwise)
            .collect::<Vec<_>>();
        assert_eq!(tokens.len(), 3);
    }
}
