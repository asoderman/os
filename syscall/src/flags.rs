use bitflags::bitflags;

bitflags! {
    #[repr(C)]
    pub struct MemoryFlags: usize {
        const READ = 1;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
        const ANONYMOUS = 1 << 3;
        const DEFAULT = MemoryFlags::READ.bits
            | MemoryFlags::WRITE.bits
            | MemoryFlags::ANONYMOUS.bits;
    }
}

bitflags! {
    #[repr(C)]
    pub struct OpenFlags: usize {
        const READ = 1;
        const WRITE = 1 << 1;
        const CREATE = 1 << 1;
    }
}

bitflags! {
    #[repr(C)]
    pub struct ReadFlags: usize {
        const NONBLOCKING = 0;
        const BLOCKING = 1;
    }
}

#[cfg(feature="staticlib")]
#[no_mangle]
pub extern "C" fn mflags_empty() -> MemoryFlags {
    MemoryFlags::READ
}
