use {
    mpchash::{HashRing, Keyspace},
    rand::random,
};
use std::ops::Deref;

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
    let ring = HashRing::new();

    let num_nodes = 10;
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        nodes.insert(i, Node { id: i as u64 });
        ring.add(nodes[i]);
    }

    let num_keys = 1000;
    for i in 0..num_keys {
        let node = ring.node(&i).unwrap();
        assert!(nodes.contains(&node));
    }
}

#[test]
fn remove_node() {
    let ring = HashRing::new();
    let node1 = Node::random();
    let node2 = Node::random();

    ring.add(node1);
    ring.add(node2);

    // Make sure that node1 and node2 are assigned to the ring.
    assert_eq!(ring.len(), 2);

    // Remove node1, and make sure that it is no longer assigned to the ring.
    ring.remove(&node1);
    assert_eq!(ring.len(), 1);
    assert_eq!(ring.node(&0).as_deref(), Some(&node2));
}

#[test]
fn duplicate_peer() {
    let ring = HashRing::new();
    let node = Node::random();
    ring.add(node);
    ring.add(node);
    assert_eq!(ring.len(), 1);
    assert_eq!(ring.node(&0).as_deref(), Some(&node));
}

#[test]
fn walkthrough() {
    // Anything that implements `Hash + Send` can be used as a node.
    // Other traits used here are derived for testing purposes.
    #[derive(Hash, Debug, PartialEq, Clone, Copy)]
    struct MyNode(u64);

    // Create a new ring, and add nodes to it.
    let ring = HashRing::new();
    ring.add(MyNode(1));
    ring.add(MyNode(2));
    ring.add(MyNode(3));
    ring.add(MyNode(4));
    ring.add(MyNode(5));

    // Anything that implements `Hash` can be used as a key.
    // To find which node should own a key:
    let key = "hello world";

    // Token is a thin wrapper holding reference to node itself
    // and to its position on the ring.
    let token = ring.node(&key).expect("empty ring");
    assert_eq!(token.position(), 1242564280540428107);
    assert_eq!(token.node(), &MyNode(2));

    // In replicated settings, we want to have several replicas
    // of a key to be stored redundantly, therefore we need multiple
    // destination/owning nodes.
    //
    // Assuming a replication factor of 3, we can do:
    let tokens = ring.replicas(&key, 3).expect("empty ring");
    assert_eq!(tokens.iter().map(|e| e.node()).collect::<Vec<_>>(), vec![
        &MyNode(1),
        &MyNode(2),
        &MyNode(3)
    ]);

    // Token can be also dereferenced to get the node itself.
    assert_eq!(tokens.iter().map(Deref::deref).collect::<Vec<_>>(), vec![
        &MyNode(1),
        &MyNode(2),
        &MyNode(3)
    ]);

    // Before node removal we probably need to move its data.
    // To find out range of keys owned by a node:
    let ranges = ring.intervals(&token).expect("empty ring");
    assert_eq!(ranges.len(), 1);

    // The range starts at the position where previous node ends,
    // and ends at the position of the owning node.
    assert_eq!(ranges[0].start, ring.position(&MyNode(1)));
    assert_eq!(ranges[0].end, ring.position(&token.node()));

    // Remove node and check the owning nodes again.
    let token_removed = ring.remove(&MyNode(2)).expect("empty ring");
    assert_eq!(token_removed.node(), &MyNode(2));

    // `MyNode(2)` is removed, `MyNode(4)` takes its place now.
    let token = ring.node(&key).expect("empty ring");
    assert_eq!(token.node(), &MyNode(4));

    let tokens = ring.replicas(&key, 3).expect("empty ring");
    assert_eq!(tokens.iter().map(|e| e.deref()).collect::<Vec<_>>(), vec![
        &MyNode(1),
        &MyNode(3),
        &MyNode(4)
    ]);
}
