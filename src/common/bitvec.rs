use alloc::vec::Vec;

trait BitExt {
    fn set_bit(&mut self, bit: usize);
    fn clear_bit(&mut self, bit: usize);
    fn get_bit(&self, bit: usize) -> bool;
}

impl BitExt for u64 {
    fn set_bit(&mut self, bit: usize) {
        *self |= 1 << bit
    }

    fn clear_bit(&mut self, bit: usize) {
        *self &= !(1 << bit)
    }

    fn get_bit(&self, bit: usize) -> bool {
        *self >> bit & 1 != 0
    }
}

#[derive(Default, Debug, Clone)]
pub struct UnorderedBitVec {
    inner: Vec<u64>,
    bits: usize,
}

impl UnorderedBitVec {
    pub fn new() -> Self {
        Self {
            inner: Vec::new(),
            bits: 0,
        }
    }

    /// Returns the first bit set to 1 that the bitmap can find. This does not adhere to any
    /// bit/byte ordering but does guarantee that entry 1 = 1, 2 = 2 ... n = n
    pub fn first_one(&self) -> Option<usize> {
        for (block_num, block) in self.inner.iter().enumerate() {
            if *block == 0 { continue; }
            for (byte_num, byte) in block.to_le_bytes().into_iter().enumerate() {
                if byte == 0 { continue; }

                let bit_num = match byte {
                    byte if byte & (1 << 7) != 0 => { 7 },
                    byte if byte & (1 << 6) != 0 => { 6 },
                    byte if byte & (1 << 5) != 0 => { 5 },
                    byte if byte & (1 << 4) != 0 => { 4 },
                    byte if byte & (1 << 3) != 0 => { 3 },
                    byte if byte & (1 << 2) != 0 => { 2 },
                    byte if byte & (1 << 1) != 0 => { 1 },
                    byte if byte & (1) != 0 => { 0 },
                    _ => unreachable!()
                };
                return Some((block_num * 64) + ((byte_num) * 8) + bit_num)
            }
        }
        None
    }

    /// The count of all bits set to 1
    pub fn count_ones(&self) -> u32 {
        self.inner.iter().map(|b| b.count_ones()).sum()
    }

    /// The equivalent of a Vec<bool>.len()
    pub fn len(&self) -> usize {
        self.bits
    }

    /// Resize Self to accomodate n bits
    pub fn resize(&mut self, bits: usize) {
        self.bits = bits;

        let mut size_in_t = bits / self.bits_per_block();
        if bits % self.bits_per_block() > 0 {
            size_in_t += 1;
        }

        self.inner.resize(size_in_t, 0);
    }

    const fn bits_per_block(&self) -> usize {
        core::mem::size_of::<u64>() * 8
    }

    /// Gets a bit at the specified index
    pub fn get(&self, idx: usize) -> Option<bool> {
        let block_idx = idx / self.bits_per_block();
        let bit_idx = idx % self.bits_per_block();

        let block = *self.inner.get(block_idx)?;

        Some(block.get_bit(bit_idx))
    }

    /// Sets a bit at the specified index
    pub fn set(&mut self, idx: usize) {
        let block_idx = idx / self.bits_per_block();
        let bit_idx = idx % self.bits_per_block();

        let block = self.inner.get_mut(block_idx).expect("Attempted to set out of bounds bit");

        block.set_bit(bit_idx);
    }

    /// Clear a bit at the specified index
    pub fn clear(&mut self, idx: usize) {
        let block_idx = idx / self.bits_per_block();
        let bit_idx = idx % self.bits_per_block();

        let block = self.inner.get_mut(block_idx).expect("Attempted to set out of bounds bit");

        block.clear_bit(bit_idx)
    }
}

#[cfg(test)]
mod test {

    use super::*;

    const TEST_SIZE: usize = 1024;

    #[test_case]
    fn test_bitmap_set_clear() {
        let mut vec = UnorderedBitVec::new();

        vec.resize(TEST_SIZE);

        for i in 0..TEST_SIZE {
            assert_eq!(vec.get(i).unwrap(), false);
            vec.set(i);
            assert_eq!(vec.get(i).unwrap(),true);
        }

        for i in 0..TEST_SIZE {
            assert_eq!(vec.get(i).unwrap(), true);
            vec.clear(i);
            assert_eq!(vec.get(i).unwrap(), false);
        }
    }

    #[test_case]
    fn test_count_ones() {
        let mut vec = UnorderedBitVec::new();

        vec.resize(TEST_SIZE);

        for i in 0..TEST_SIZE {
            vec.set(i);
            assert_eq!(vec.count_ones() as usize, i + 1);
        }

        assert_eq!(vec.count_ones() as usize, TEST_SIZE);

    }

    #[test_case]
    fn test_get_first() {
        let mut vec = UnorderedBitVec::new();
        vec.resize(TEST_SIZE);

        // Test single bit ascending
        for i in 0..TEST_SIZE {
            vec.set(i);
            assert_eq!(vec.first_one().unwrap(), i);
            vec.clear(i);
        }

        // Test single bit descending
        for i in (0..TEST_SIZE).rev() {
            vec.set(i);

            assert_eq!(vec.first_one().unwrap(), i);
            vec.clear(i);
        }
    }
}
