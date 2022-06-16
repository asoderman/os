use alloc::vec::Vec;
use bitfield::{Bit, BitRange};

#[derive(Default, Debug, Clone)]
pub struct BitVec<T = u64> {
    inner: Vec<T>,
    bits: usize,
}

impl<T: Clone + Default + BitRange<u8>> BitVec<T> {
    pub fn new() -> Self {
        Self {
            inner: Vec::new(),
            bits: 0,
        }
    }

    pub fn resize(&mut self, bits: usize) {
        let mut size_in_t = bits / self.bits_per_block();
        if bits % self.bits_per_block() > 0 {
            size_in_t += 1;
        }

        self.inner.resize(size_in_t, T::default());
    }

    fn bits_per_block(&self) -> usize {
        core::mem::size_of::<T>() * 8
    }

    pub fn get(&self, idx: usize) -> Option<bool> {
        let block_idx = idx / core::mem::size_of::<T>();
        let bit_idx = idx % core::mem::size_of::<T>();

        let block = *self.inner.get(block_idx)?;

        Some(block.bit(bit_idx))
    }

    pub fn set(&mut self, idx: usize, value: bool) {
        let block_idx = idx / self.bits_per_block();
        let bit_idx = idx % self.bits_per_block();

        let block = self.inner.get_mut(block_idx).expect("Attempted to set out of bounds bit");

        block.set_bit(bit_idx, value);
    }
}
