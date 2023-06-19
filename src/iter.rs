/// Defines an iterator over a range of ring token positions.
pub(crate) enum HashRingIter<T, U> {
    Clockwise(T),
    CounterClockwise(U),
}

impl<T, U, V> Iterator for HashRingIter<T, U>
where
    T: Iterator<Item = V>,
    U: Iterator<Item = V>,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Clockwise(iter) => iter.next(),
            Self::CounterClockwise(iter) => iter.next(),
        }
    }
}

impl<T, U, V> DoubleEndedIterator for HashRingIter<T, U>
where
    T: Iterator<Item = V> + DoubleEndedIterator,
    U: Iterator<Item = V> + DoubleEndedIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Clockwise(iter) => iter.next_back(),
            Self::CounterClockwise(iter) => iter.next_back(),
        }
    }
}
