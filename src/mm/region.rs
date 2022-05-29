/// A trait containing helper functions for comparing regions of memory (Physical or Virtual)
pub trait MemRegion {
    fn region_start(&self) -> usize;
    fn region_end(&self) -> usize;

    /// The size of the region.
    fn size(&self) -> usize {
        (self.region_end() - self.region_start()) as usize
    }

    /// A helper function that returns if self is within other
    fn within(&self, other: &Self) -> bool {
        self.region_start() >= other.region_start() && self.region_end() <= other.region_end()
    }

    /// A helper that returns whether or not other is a subregion of self.
    fn contains(&self, other: &Self) -> bool {
        other.region_start() >= self.region_start() && other.region_end() <= self.region_end()
    }

    /// A helper that returns if the value lies within the range
    fn contains_val(&self, value: usize) -> bool {
        self.region_start() <= value && value < self.region_end()
    }

    /// A helper function that returns if self overlaps the other region at all
    fn overlaps(&self, other: &Self) -> bool {
        let end_overhang = other.contains_val(self.region_start()) && self.region_end() > other.region_end();
        // self.end() is the exclusive end
        if self.region_end() == 0 {
            crate::println!("self.end = {:X}", self.region_end());
            crate::println!("self.start = {:X}", self.region_start());
        }

        let start_overhang = self.region_start() < other.region_start() && other.contains_val(self.region_end() - 1);

        start_overhang || end_overhang
    }

    /// Returns if the two regions are contiguous. Order does not matter.
    fn contiguous(&self, other: &Self) -> bool {
        self.region_start() == other.region_end() || other.region_start() == self.region_end()
    }
}
