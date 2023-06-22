use mpchash::{DefaultPartitioner, Partitioner, Xxh3Partitioner};

#[test]
fn default_partitioner() {
    let partitioner = DefaultPartitioner::new();
    assert_eq!(partitioner.position(&0u64), 0x1424aa9885a1d5c7);
    assert_eq!(partitioner.position(&1u64), 0xcfb732e08be9ec0);
    assert_eq!(partitioner.position(&123456u64), 0x4879daae426e14fb);
}

#[test]
fn position() {
    let partitioner = Xxh3Partitioner::new();
    assert_eq!(partitioner.position(&0u64), 0x1424aa9885a1d5c7);
    assert_eq!(partitioner.position(&1u64), 0xcfb732e08be9ec0);
    assert_eq!(partitioner.position(&123456u64), 0x4879daae426e14fb);
}

#[test]
fn position_seeded() {
    let partitioner = Xxh3Partitioner::new();
    assert_eq!(partitioner.position_seeded(&0u64, 0), 0xc77b3abb6f87acd9);
    assert_eq!(partitioner.position_seeded(&1u64, 0), 0x2fbc593564db792e);
    assert_eq!(
        partitioner.position_seeded(&123456u64, 0),
        0xf3088e53441a77c5
    );
    assert_eq!(partitioner.position_seeded(&0u64, 1), 0x9e51ad6d2f3e695c);
    assert_eq!(partitioner.position_seeded(&1u64, 1), 0x85671091eda75eb5);
    assert_eq!(
        partitioner.position_seeded(&123456u64, 1),
        0x75a073dcf2e9322a
    );
}
