use {
    mpchash::{
        HashRing,
        RingDirection::{self, Clockwise, CounterClockwise},
    },
    rand::Rng,
    std::collections::BTreeSet,
};

#[derive(Hash, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Node {
    id: u64,
}

impl Node {
    fn random() -> Self {
        Self {
            id: rand::thread_rng().gen(),
        }
    }
}

#[test]
fn add_node() {
    let mut ring = HashRing::new();

    let num_nodes = 10;
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        nodes.insert(i, Node { id: i as u64 });
        ring.add(nodes[i]);
    }

    let num_keys = 1000;
    for i in 0..num_keys {
        let node = ring.primary_node(&i).unwrap();
        assert!(nodes.contains(node));
    }
}

#[test]
fn remove_node() {
    let mut ring = HashRing::new();
    let node1 = Node::random();
    let node2 = Node::random();

    ring.add(node1);
    ring.add(node2);

    // Make sure that node1 and node2 are assigned to the ring.
    assert_eq!(ring.len(), 2);

    // Remove node1, and make sure that it is no longer assigned to the ring.
    ring.remove(&node1);
    assert_eq!(ring.len(), 1);
    assert_eq!(ring.primary_node(&0), Some(&node2));
}

#[test]
fn duplicate_peer() {
    let mut ring = HashRing::new();
    let node = Node::random();
    ring.add(node);
    ring.add(node);
    assert_eq!(ring.len(), 1);
    assert_eq!(ring.primary_node(&0), Some(&node));
}

#[test]
fn tokens() {
    let mut ring = HashRing::new();
    let node1 = Node::random();
    let node2 = Node::random();
    let node3 = Node::random();
    ring.add(node1);
    ring.add(node2);
    ring.add(node3);

    // Traverse from the beginning (clockwise).
    let positions = ring
        .tokens(0, Clockwise)
        .map(|token| *token.1)
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 3);
    assert!(positions.contains(&node1));
    assert!(positions.contains(&node2));
    assert!(positions.contains(&node3));

    // Traverse from the beginning (counter-clockwise).
    let positions = ring
        .tokens(0, CounterClockwise)
        .map(|token| *token.1)
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 3);
    assert!(positions.contains(&node1));
    assert!(positions.contains(&node2));
    assert!(positions.contains(&node3));

    // Unregister peer2, and make sure that it is no longer assigned to the ring.
    ring.remove(&node2);
    let positions = ring
        .tokens(0, Clockwise)
        .map(|token| *token.1)
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 2);
    assert!(positions.contains(&node1));
    assert!(!positions.contains(&node2));
    assert!(positions.contains(&node3));
}

#[test]
fn tokens_wrap_around() {
    let mut ring = HashRing::new();
    let nodes = vec![Node::random(), Node::random(), Node::random()];
    nodes.iter().for_each(|node| ring.add(*node));

    // Start from position near the end of the ring (wrap around, clockwise).
    let positions = ring
        .tokens(u64::MAX - 1, Clockwise)
        .map(|token| *token.1)
        .collect::<Vec<_>>();
    assert_eq!(
        BTreeSet::from_iter(positions),
        BTreeSet::from_iter(nodes.clone())
    );

    // Start from position near zero of the ring (wrap around, counter-clockwise).
    let positions = ring
        .tokens(1, CounterClockwise)
        .map(|token| *token.1)
        .collect::<Vec<_>>();
    assert_eq!(BTreeSet::from_iter(positions), BTreeSet::from_iter(nodes));
}

#[track_caller]
fn assert_nodes(ring: &HashRing<Node>, start: u64, dir: RingDirection, expected: Vec<Node>) {
    let positions = ring
        .tokens(start, dir)
        .map(|token| *token.1)
        .collect::<Vec<_>>();
    assert_eq!(positions, expected);
}

#[test]
fn tokens_corner_cases() {
    let mut ring = HashRing::new();
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
    let mut ring = HashRing::new();
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
