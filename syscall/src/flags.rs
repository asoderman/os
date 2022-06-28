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

#[cfg(feature="staticlib")]
#[no_mangle]
pub extern "C" fn mflags_empty() -> MemoryFlags {
    MemoryFlags::READ
}
