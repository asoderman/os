/// A trait containing helper functions for comparing regions of memory (Physical or Virtual)
pub trait Region {
    fn start(&self) -> usize;
    fn end(&self) -> usize;

    /// The size of the region.
    fn size(&self) -> usize {
        (self.end() - self.start()) as usize
    }

    /// A helper function that returns if self is within other
    fn within(&self, other: &Self) -> bool {
        self.start() >= other.start() && self.end() <= other.end()
    }

    /// A helper that returns whether or not other is a subregion of self.
    #[inline]
    fn contains(&self, other: &Self) -> bool {
        other.start() >= self.start() && other.end() <= self.end()
    }

    /// A helper that returns if the value lies within the range
    fn contains_val(&self, value: usize) -> bool {
        self.start() <= value && value < self.end()
    }

    /// A helper function that returns if self overlaps the other region at all
    fn overlaps(&self, other: &Self) -> bool {
        let end_overhang = other.contains_val(self.start()) && self.end() > other.end();
        let start_overhang = self.start() < other.start() && other.contains_val(self.end());

        start_overhang || end_overhang
    }

    /// Returns if the two regions are contiguous. Order does not matter.
    fn contiguous(&self, other: &Self) -> bool {
        self.start() == other.end() || other.start() == self.end()
    }
}
