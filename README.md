# mpchash

[![crates.io](https://img.shields.io/crates/d/mpchash.svg)](https://crates.io/crates/mpchash) [![docs.rs](https://docs.rs/mpchash/badge.svg)](https://docs.rs/mpchash)

Multi-probe consistent hashing implementation based on [this paper](https://arxiv.org/pdf/1505.00062.pdf).

## Features

- [x] Multi-probe consistent hashing.
- [x] Uniform distribution, with no virtual nodes, so no extra space required. The high space requirement is the main
  downside of the Karger's ring.
- [x] All conventional consistent hashing features, like moving through the ring (in both directions), adding and
  removing nodes, finding the closest node to a key, etc.

## Usage

Define a type that implements the `RingNode`
trait<sup>1</sup> as a node or use some existing type, like `u64` (for which blanket implementation is provided):

<sup>1</sup>`Hash` + `Clone` + `Copy` + `Debug` + `Eq` + `PartialEq` + `Ord` + `PartialOrd`.

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

impl mpchash::RingNode for Node {}
```

Then, create a `Ring` and populate it with nodes:

```rust
fn main() {
  let mut ring = mpchash::Ring::new();
  nodes.add(Node::new(1));
  nodes.add(Node::new(2));
  nodes.add(Node::new(3));
}
```

Anything that implements `Hash` can be used as a key:

```rust
fn main() {
  let key = "hello world";

  // Get the closest, when moving in CW direction, node to the key.
  // That node is assumed as "owning" the key space for the key.
  let node = ring.primary_node(&key);

  // If we are interested in both ring position and owning node, 
  // we can get them with `primary_token`. A token is just a tuple 
  // of `(position, node)`.
  let token = ring.primary_token(&key);
}
```

In replicated settings, we want to have several replicas of a key, so need multiple destination nodes.
In order to obtain such replica nodes, we can traverse the ring from a given position:

```rust
fn main() {
  let tokens = ring
          .tokens(ring.position(&"foo"), Clockwise)
          .collect::<Vec<_>>();
}
```

## License

MIT