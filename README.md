# mpchash

[![crates.io](https://img.shields.io/crates/d/mpchash.svg)](https://crates.io/crates/mpchash) [![docs.rs](https://docs.rs/mpchash/badge.svg)](https://docs.rs/mpchash)

Consistent hashing algorithm implementation based
on the [Multi-probe consistent hashing](https://arxiv.org/pdf/1505.00062.pdf) paper.

## Features

- [x] Multi-probe consistent hashing.
- [x] Balanced distribution of keys, with peak-to-average load ratio of `1 + ε` with just `1 + 1/ε` lookups per key.
- [x] No virtual nodes, so no extra space required -- `O(n)` space complexity. The high space requirement is the main
  downside of the original Karger's ring.
- [x] All conventional consistent hashing methods, like moving through the ring (in both directions), adding and
  removing nodes, finding the closest node to a key, finding a key range owned by a node etc.

## Usage

The implementation supports all the features of the conventional consistent hashing algorithm, so it can be used as a
drop-in replacement for any existing implementation.

### Defining a node

Anything that implements the following traits can be placed on a ring as a node:

`Hash` + `Clone` + `Debug` + `Eq` + `PartialEq` + `Ord` + `PartialOrd`

Here is an example of using custom type as a node:

```rust
#[derive(Hash, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Node {
    id: u64,
}

impl Node {
    fn new(id: u64) -> Self {
        Self { id }
    }
}
```

### Creating a ring

To create a ring and populate it with nodes:

```rust
fn main() {
    use mpchash::HashRing;

    let mut ring = HashRing::new();
    ring.add(Node::new(1));
    ring.add(Node::new(2));
    ring.add(Node::new(3));
}
```

### Finding a node that owns a key

Anything that implements `Hash` can be used as a key:

```rust
fn main() {
    // ... ring initialization code

    // Anything that implements `Hash` can be used as a key.
    let key = "hello world";

    // Node that owns the key.
    //
    // It is the first node when moving in CW direction from the 
    // position where key is hashed to.
    let owning_node = ring.primary_node(&key);

    // If we are interested in both ring position and owning node, 
    // we can get them with `primary_token`. 
    // 
    // A token is just a tuple of `(position, node)`.
    let token = ring.primary_token(&key);
}
```

In replicated settings, we want to have several replicas of a key, so need multiple destination/owning nodes.

In order to obtain such a list of replica nodes, we can traverse the ring from a given position:

```rust
fn main() {
    use mpchash::HashRing;
    use mpchash::RingDirection::Clockwise;

    let mut ring = HashRing::new();
    ring.add(Node::new(1));
    ring.add(Node::new(1));

    let key = "hello world";
    let tokens = ring
        .tokens(ring.position(&key), Clockwise)
        .collect::<Vec<_>>();

    for (pos, node) in ring.tokens(&key, Clockwise) {
        println!("node {} is at position {}", node, pos);
    }
}
```

Normally, you would collect/traverse `replication factor` number of tokens, so that you have `replication factor` of
destination nodes.

### Finding a key range owned by a node

Sometimes it is necessary to find a range of keys owned by a node. For example, when some node's data needs to be
rebalanced to another node. In this case, we are moving from the node's position in the ring in CCW direction, until we
find a previous node. As we are operating on a ring we need to account for the wrap-around. All this is handled by
the `key_range` method:

```rust
fn main() {
    use mpchash::RingPosition;
    use mpchash::HashRing;

    let mut ring = HashRing::new();

    // Define nodes.
    let node1 = "SomeNode1";
    let node2 = "SomeNode2";

    // Add nodes to the ring.
    ring.add(node1);
    ring.add(node2);

    // Get the range owned by node1.
    let pos = ring.position(&node1);
    let range = ring.key_range(pos).unwrap();

    // The range starts at the position to the left of node1,
    // till (and not including) its own position.
    assert_eq!(range.start, ring.position(&node2));
    assert_eq!(range.end, ring.position(&node1));
}
```

## License

MIT