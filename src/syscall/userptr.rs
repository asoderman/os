use syscall::error::SyscallError;
use crate::arch::VirtAddr;

pub struct UserPtr(VirtAddr);

impl UserPtr {
    pub fn new(addr: VirtAddr) -> Result<Self, SyscallError> {
        if addr.as_u64() < 0xFFFFFF8000000000 {
            Ok(Self(addr))
        } else {
            Err(SyscallError::InvalidPtr)
        }
    }

    pub fn addr(&self) -> VirtAddr {
        self.0
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub unsafe fn as_mut<T>(&mut self) -> Option<&mut T> {
        (self.0.as_mut_ptr() as *mut T).as_mut()
    }

    pub unsafe fn as_raw_mut<T>(&mut self) -> *mut T {
        self.0.as_mut_ptr() as *mut T
    }

    pub unsafe fn read_bytes(&self, len: usize) -> &[u8] {
        let ptr = self.0.as_ptr() as *const u8;

        core::slice::from_raw_parts(ptr, len)
    }

    pub unsafe fn write_bytes(&self, len: usize) -> &mut [u8] {
        let ptr = self.0.as_mut_ptr() as *mut u8;

        core::slice::from_raw_parts_mut(ptr, len)
    }
}

impl TryFrom<usize> for UserPtr {
    type Error = SyscallError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        let vaddr = VirtAddr::new(value as u64);
        Self::new(vaddr)
    }
}
