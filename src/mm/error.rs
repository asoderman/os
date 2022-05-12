use alloc::boxed::Box;
use core::fmt::Debug;

use super::vmm::VirtualMemoryError;
use crate::error::Error;

#[derive(Debug)]
pub struct MemoryManagerError(Box<dyn Error>);

impl Error for MemoryManagerError {
    fn source(&self) -> Option<&Box<dyn Error>> {
        Some(&self.0)
    }
}

impl From<VirtualMemoryError> for MemoryManagerError {
    fn from(e: VirtualMemoryError) -> Self {
        MemoryManagerError(Box::new(e))
    }
}

impl Error for VirtualMemoryError {
    fn source(&self) -> Option<&Box<dyn Error>> {
        None
    }
}

