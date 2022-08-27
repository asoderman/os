use core::sync::atomic::{AtomicU32, Ordering};

use alloc::{vec::Vec, sync::Arc};
use spin::Mutex;
use syscall::flags::OpenFlags;

use crate::arch::PAGE_SIZE;

use super::{file::{Read, Write, File}, FsError, Path, rootfs};

#[derive(Debug, Clone, Copy)]
pub enum FifoDirection {
    Reader,
    Writer
}

/// The backing store for the FIFO
#[derive(Debug, Default)]
struct FifoInner {
    pub data: Mutex<Vec<u8>>,
    pub reader_count: AtomicU32,
    pub writer_count: AtomicU32,
}

impl FifoInner {
    pub fn new() -> Self {
        Self::default()
    }

    fn increment_reader(&self) {
        self.reader_count.fetch_add(1, Ordering::SeqCst);
    }

    fn decrement_reader(&self) {
        self.reader_count.fetch_sub(1, Ordering::SeqCst);
    }

    fn increment_writer(&self) {
        self.writer_count.fetch_add(1, Ordering::SeqCst);
    }

    fn decrement_write(&self) {
        self.writer_count.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug)]
pub struct Fifo {
    fifo: Arc<FifoInner>,
    direction: FifoDirection,
}

impl Fifo {
    /// Creates a new fifo and return handle with the provided direction
    pub fn new_with_direction(direction: FifoDirection) -> Fifo {
        let fifo = Arc::new(FifoInner::new());

        match direction {
            FifoDirection::Writer => fifo.increment_writer(),
            FifoDirection::Reader => fifo.increment_reader(),
        };

        Fifo {
            fifo,
            direction,
        }
    }

    /// Clone an existing fifo handle with specified direction
    pub fn clone_with_direction(&self, direction: FifoDirection) -> Self {
        Self {
            fifo: Arc::clone(&self.fifo),
            direction
        }
    }
}

impl Read for Fifo {
    fn read(&self, buf: &mut [u8]) -> Result<usize, super::Error> {
        let mut lock = self.fifo.data.lock();

        let mut bytes_read = 0;
        let available = lock.len();

        if available >= buf.len() {
            buf.copy_from_slice(lock.drain(0..buf.len()).as_slice());
            bytes_read = buf.len();
        } else if available < buf.len() {
            buf.copy_from_slice(lock.drain(0..available).as_slice());
            bytes_read = available
        }

        Ok(bytes_read)
    }
}

impl Write for Fifo {
    fn write(&mut self, buf: &[u8]) -> Result<usize, super::Error> {
        let mut lock = self.fifo.data.lock();


        lock.extend_from_slice(buf);
        // TODO: check write does not exceed max pipe/write size
        assert!(lock.len() < PAGE_SIZE, "FIFO too big!");
        Ok(buf.len())
    }
}

impl File for Fifo {
    fn content(&self) -> Result<&[u8], super::Error> {
        Err(FsError::InvalidAccess)
    }

    fn position(&self) -> usize {
        0
    }

    fn attributes(&self) -> super::file::FileAttributes {
        todo!()
    }

    fn close(&self) -> Result<(), super::Error> {
        todo!("Implement FIFO close")
    }
}
