use super::{Error, FsError};

use alloc::string::String;

/// A Path type that wraps a heap allocated string and defines some common path operations
#[derive(Debug, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub struct Path(String);

impl Path {
    /// Construct a `Path` via the provided utf8 byte buffer
    ///
    /// # Returns
    /// `Path` on success
    /// `FsError` wrapped `Utf8Error` on error
    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        let string = String::from_utf8(bytes.to_vec()).map_err(|e| FsError::InvalidPath(Some(e)))?;

        Ok(Self(string))
    }

    /// Constructs a path from a string slice
    ///
    /// # Safety
    /// This unwrap is safe because the string slice should already be a valid utf8 buffer
    pub fn from_str(path: &str) -> Self {
        Path::new(path.as_bytes()).unwrap()
    }

    /// Constructs an empty ("") path
    pub fn empty() -> Self {
        Self::from_str("")
    }

    /// Join two `Path`s together returning a newly allocated `Path`
    pub fn join(&self, other: &Self) -> Self {
        let mut new_path = self.clone();
        new_path.append(other);
        new_path
    }

    /// Appends other to self in place. This should function the same as `Path::join` but without a
    /// new allocation
    pub fn append(&mut self, other: &Self) {
        if !self.0.ends_with('/') {
            self.0.push('/');
        }

        // If thw other path starts with a separator index around it
        let start_index = if other.0.starts_with('/') {
            1
        } else {
            0
        };

        self.0.push_str(&other.0.as_str()[start_index..]);
    }

    /// Returns an iterator of each node that makes up the path
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    pub(super) fn starts_with(&self, prefix: &Self) -> bool {
        self.0.starts_with(&prefix.0)
    }

    /// Attempts to return the last component of the path however this does NOT perform a check whether or not the
    /// return value is a file versus a directory
    pub fn filename(&self) -> Option<&str> {
        self.components().last()
    }

    /// Checks if the path is relative
    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }

    /// Checks if the path begins with a separator implying the path is absolute. The path could
    /// still be relative to the filesystem root
    pub fn is_absolute(&self) -> bool {
        self.0.starts_with('/')
    }
}
