use crate::arch::PhysAddr;

pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> PhysAddr;
    fn deallocate_frame(&mut self, frame: PhysAddr);
}
