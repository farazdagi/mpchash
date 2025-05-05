# mpchash

[![crates.io](https://img.shields.io/crates/d/mpchash.svg)](https://crates.io/crates/mpchash)
[![docs.rs](https://docs.rs/mpchash/badge.svg)](https://docs.rs/mpchash)

Consistent hashing algorithm implementation based on the
[Multi-probe consistent hashing](https://arxiv.org/pdf/1505.00062.pdf) paper.

## Features

- [x] Balanced distribution of keys, with peak-to-average load ratio of `1 + ε` with just `1 + 1/ε`
  lookups per key.
- [x] No virtual nodes, so no extra space required -- `O(n)` space complexity. The high space
  requirement is the main downside of the original
  [Karger's ring](https://dl.acm.org/doi/10.1145/258533.258660).

## Motivation

The original consistent hashing algorithm was introduced by Karger et al. in the
[Consistent Hashing and Random Trees](https://dl.acm.org/doi/10.1145/258533.258660) paper. While the
algorithm provides number of very useful properties, it has a very important limitation: nodes
positions are assigned pseudo-randomly and since there normally not that many physical nodes to
begin with -- the key space is rarely partitioned evenly, with some nodes controlling bigger
segments of the ring.

The conventional solution is to introduce virtual nodes, which are replicas of the physical nodes.
This way, there are way more node points on the ring, and, as the result, the key space is divided
more evenly. The downside of this approach is higher memory requirements to store the ring state.
This also complicates the ring state management a bit.

Multi-probe consistent hashing resolves this problem.

## Usage

The implementation supports all the necessary methods for hash ring management:

``` rust
use mpchash::{HashRing, Keyspace};
use std::ops::Deref;

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
let tokens = ring.replicas(&key, 3);
assert_eq!(tokens, vec![&MyNode(1), &MyNode(2), &MyNode(3)]);

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

let tokens = ring.replicas(&key, 3);
assert_eq!(tokens.iter().map(|e| e.node()).collect::<Vec<_>>(), vec![
    &MyNode(1),
    &MyNode(3),
    &MyNode(4)
]);
```

## Implementation Notes

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

See the [paper](https://arxiv.org/pdf/1505.00062.pdf) for more details.

## License

MIT
