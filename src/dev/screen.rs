use core::ptr::NonNull;

pub type RGBA = u32;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: usize,
    pub y: usize,
}

pub struct Screen<P: Sized> {
    frame_buffer_ptr: NonNull<P>,
    buffer_size: usize,
    width: usize,
    height: usize,
}

impl<P: Sized + Copy> Screen<P> {
    pub fn new(buffer: *mut P, size: usize, width: usize, height: usize) -> Self {
        Screen {
            frame_buffer_ptr: NonNull::new(buffer).unwrap(),
            buffer_size: size,
            width,
            height,
        }
    }

    pub fn draw_pixel(&mut self, x: usize, y: usize, color: P) {
        // TODO: use stride
        let buffer_offset = y * self.width + x;
        assert!(buffer_offset < self.buffer_size);
        unsafe {
            core::ptr::write(self.frame_buffer_ptr.as_ptr(), color);
        }
    }

    pub fn fill_screen(&mut self, color: P) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.draw_pixel(x, y, color);
            }
        }
    }
}
