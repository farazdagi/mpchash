use {
    crate::{RingNode, RingPosition},
    crossbeam_skiplist::map::Entry,
    std::{borrow::Borrow, ops::Deref},
};

/// An ownership over a position on the ring by the object of type `T`
/// (normally, `RingNode`).
///
/// Wrapper around `crossbeam_skiplist::map::Entry` that allows to obtain
/// entry's key/value as references.
#[derive(Clone, Debug)]
pub struct RingToken<'a, T>(Entry<'a, RingPosition, T>);

impl<T: RingNode> RingToken<'_, T> {
    /// Return the position of the node on the ring.
    pub fn position(&self) -> RingPosition {
        *self.0.key()
    }

    /// Return the node that owns this token.
    pub fn node(&self) -> &T {
        self.0.value()
    }
}

impl<T> Deref for RingToken<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.value()
    }
}

impl<T> AsRef<T> for RingToken<'_, T> {
    fn as_ref(&self) -> &T {
        self.0.value()
    }
}

impl<T> Borrow<T> for RingToken<'_, T> {
    fn borrow(&self) -> &T {
        self.0.value()
    }
}

impl<'a, T> From<Entry<'a, RingPosition, T>> for RingToken<'a, T> {
    fn from(entry: Entry<'a, RingPosition, T>) -> Self {
        Self(entry)
    }
}

impl<T: RingNode> PartialEq for RingToken<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.position() == other.position()
    }
}

impl<T: RingNode> Eq for RingToken<'_, T> {}

impl<T> PartialEq<T> for RingToken<'_, T>
where
    T: RingNode + PartialEq,
{
    fn eq(&self, other: &T) -> bool {
        self.node() == other
    }
}

impl<T> PartialEq<&T> for RingToken<'_, T>
where
    T: RingNode + PartialEq,
{
    fn eq(&self, other: &&T) -> bool {
        self.node() == *other
    }
}

impl<T: RingNode> PartialOrd for RingToken<'_, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: RingNode> Ord for RingToken<'_, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.position().cmp(&other.position())
    }
}
