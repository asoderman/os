#[derive(Debug)]
#[repr(isize)]
pub enum SyscallError {
    /// No syscal available
    NoSys = -1,
    /// Ptr was unacceptable for the operation.
    InvalidPtr = -2,
    /// Memeory mapping exists and could not overwrite
    Exist = -3,
    /// Memeory mapping does not exists or OOM
    NoMem = -4,
    InvalidFlags = -5,
    InvalidPath = -6,
    InvalidFd = -7,
    FsError = -8,
}

pub const OK_VAL: isize = 0;
