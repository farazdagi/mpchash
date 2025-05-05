use {
    mpchash::{
        HashRing,
        Keyspace,
    },
    rand::random,
};

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
