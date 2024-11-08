# mpchash

[![crates.io](https://img.shields.io/crates/d/mpchash.svg)](https://crates.io/crates/mpchash)
[![docs.rs](https://docs.rs/mpchash/badge.svg)](https://docs.rs/mpchash)

Consistent hashing algorithm implementation based on the
[Multi-probe consistent hashing](https://arxiv.org/pdf/1505.00062.pdf) paper.

## Features

- [x] Multi-probe consistent hashing.
- [x] Balanced distribution of keys, with peak-to-average load ratio of `1 + ε` with just `1 + 1/ε`
  lookups per key.
- [x] No virtual nodes, so no extra space required -- `O(n)` space complexity. The high space
  requirement is the main downside of the original Karger's ring.
- [x] All conventional consistent hashing methods, like moving through the ring (in both
  directions), adding and removing nodes, finding the closest node to a key, finding a key range
  owned by a node etc.

## Motivation

The original consistent hashing algorithm was introduced by Karger et al. (in the
[Consistent Hashing and Random Trees: Distributed Caching Protocols for Relieving Hot Spots on the World Wide Web](https://dl.acm.org/doi/10.1145/258533.258660)
paper) and while it provides number of very useful properties, it has a very important limitation:
as nodes are assigned positions on the ring using the hash value of their identifiers, and since
there normally not that many physical nodes to begin with -- the key space is rarely partitioned
evenly, with some nodes controlling bigger segments of the ring.

The conventional solution is to introduce virtual nodes, which are replicas of the actual nodes, and
assign them positions on the ring. This way, there are way more node points on the ring, and thus
the key space is divided more evenly among the nodes. The downside of this approach is higher memory
requirements to store the ring state. This also complicates the ring state management a bit.

Multi-probe consistent hashing is a variant of consistent hashing that doesn't require introduction
of virtual nodes, yet achieves very similar load balancing properties.

Nodes are still assigned positions on the ring based on hash values of their identifiers, however,
instead of assigning the key to the next clockwise node, multiple probes are made (using
double-hashing), and the attempt with the closest distance to some node wins -- that node is
considered owning the key space for the key.

Unevenly sliced key space is not a problem anymore: indeed, the bigger the segment of the key space
owned by a node is, the lesser the chance that a probe for a key will land close enough to that
node's position, and thus be assigned to it. So, nodes with bigger segments have their chances
increased for a probe to land on their segments, however, due to very size of the segment -- the
chance of landing close enough to the node point, and thus being eventually picked as a successful
probe, are lowered.

See the paper for more details.

## Usage

The implementation supports all the features of the conventional consistent hashing algorithm, so it
can be used as a drop-in replacement for any existing implementation.

### Defining a node

Anything that implements the following traits can be placed on a ring as a node:

`Hash` + `Clone` + `Debug` + `Eq` + `PartialEq` + `Ord` + `PartialOrd`

Here is an example of using custom type as a node:

``` rust
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

``` rust
use mpchash::HashRing;

# #[derive(Hash, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
# struct Node {
#    id: u64,
# }
#
# impl Node {
#    fn new(id: u64) -> Self {
#        Self { id }
#    }
# }
fn main() {
    let mut ring = HashRing::new();
    ring.add(Node::new(1));
    ring.add(Node::new(2));
    ring.add(Node::new(3));
}
```

### Finding a node that owns a key

Anything that implements `Hash` can be used as a key:

``` rust
use mpchash::HashRing;

# #[derive(Hash, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
# struct Node {
#    id: u64,
# }
#
# impl Node {
#    fn new(id: u64) -> Self {
#        Self { id }
#    }
# }
fn main() {
#   let mut ring = HashRing::new();
#   ring.add(Node::new(1));
#   ring.add(Node::new(2));
#   ring.add(Node::new(3));
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

In replicated settings, we want to have several replicas of a key, so need multiple
destination/owning nodes.

In order to obtain such a list of replica nodes, we can traverse the ring from a given position:

``` rust
use mpchash::{HashRing, RingDirection::Clockwise};

# #[derive(Hash, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
# struct Node {
#    id: u64,
# }
#
# impl Node {
#    fn new(id: u64) -> Self {
#        Self { id }
#    }
# }
fn main() {
    let mut ring = HashRing::new();
    ring.add(Node::new(1));
    ring.add(Node::new(1));

    let key = "hello world";
    let tokens = ring
        .tokens(ring.position(&key), Clockwise)
        .collect::<Vec<_>>();

    for (pos, node) in ring.tokens(ring.position(&key), Clockwise) {
        println!("node {:?} is at position {:?}", node, pos);
    }
}
```

Normally, you would collect/traverse `replication factor` number of tokens, so that you have
`replication factor` of destination nodes.

### Finding a key range owned by a node

Sometimes it is necessary to find a range of keys owned by a node. For example, when some node's
data needs to be rebalanced to another node. In this case, we are moving from the node's position in
the ring in CCW direction, until we find a previous node. As we are operating on a ring we need to
account for the wrap-around. All this is handled by the `key_range` method:

``` rust
use mpchash::{HashRing, RingDirection::Clockwise};

fn main() {
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
