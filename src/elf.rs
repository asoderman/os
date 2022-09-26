use log::{info, trace};
use xmas_elf::program::Type;
use xmas_elf::{ElfFile, header::Header};

use crate::arch::{VirtAddr, PAGE_SIZE};
use crate::mm::{user_map, MemoryManagerError};
use crate::proc::Task;
use crate::stack::UserStack;

const USER_STACK_ADDR: u64 = 0x00f000;
const USER_STACK_SIZE: usize = 4;

#[derive(Debug)]
pub enum LoaderError {
    Lib(&'static str)
}

impl From<&'static str> for LoaderError {
    fn from(msg: &'static str) -> Self {
        LoaderError::Lib(msg)
    }
}

pub trait Loader {
    fn load_elf(&mut self, _: &[u8]) -> Result<(), LoaderError>;
}

impl Loader for Task {
    fn load_elf(&mut self, data: &[u8]) -> Result<(), LoaderError> {
        let elf_file = ElfFile::new(data)?;

        // store the entry_point
        self.entry_point = entry_point(&elf_file.header);
        info!("entry point: {:?}", self.entry_point);

        // Map the stack for usermode
        let user_stack_base = VirtAddr::new(USER_STACK_ADDR);
        user_map(self, user_stack_base, USER_STACK_SIZE).unwrap();
        self.user_stack = Some(UserStack::new(user_stack_base, USER_STACK_SIZE));

        // Write the data
        for p_header in elf_file.program_iter() {
            match p_header.get_type()? {
                Type::Load => {
                    // Load program sections

                    let mem_size = p_header.mem_size() as usize;
                    info!("mem_size: {:X}", mem_size);

                    let header_addr = VirtAddr::new(p_header.virtual_addr());
                    let virt_offset =  (header_addr.as_u64() - header_addr.as_u64()) as usize;
                    let virt_end = virt_offset as usize + mem_size;

                    let crosses_page_boundary = if header_addr.as_u64() as usize / PAGE_SIZE < (header_addr + mem_size).as_u64() as usize / PAGE_SIZE { 1 } else { 0 };

                    // round up and add to end if page boundary is crossed
                    let pages = ((mem_size + (PAGE_SIZE - 1)) / PAGE_SIZE) + crosses_page_boundary;

                    info!("Mapping {} pages for elf", pages);

                    // FIXME: allow overlapping mappings since headers can exist within the same page
                    match user_map(self, header_addr.align_down(0x1000u64), pages) {
                        Ok(_) => (),
                        Err(MemoryManagerError(e)) => {
                            log::warn!("{:?}", e);
                            log::warn!("Elf loader memory map overlap");
                        },
                        _ => panic!("Map error in elf loader"),
                    }

                    trace!("v_offset: {:X} v_end: {:X}", virt_offset, virt_end);
                    let f_offset =  p_header.offset() as usize;
                    let f_end = f_offset + mem_size;

                    trace!("f_offset: {:X} f_end: {:X}", f_offset, f_end);

                    let header_ptr: *mut u8 = header_addr.as_mut_ptr();
                    let mut byte_count = 0;

                    // rust gets cranky if we use a null ptr in a slice
                    if header_ptr.is_null() {
                        for byte in &data[f_offset..f_end] {
                            unsafe {
                                header_ptr.add(byte_count).write(*byte);
                                byte_count += 1;
                            }
                        }
                    } else {
                        let header_slice = unsafe {
                            core::slice::from_raw_parts_mut(header_ptr, p_header.file_size() as usize)
                        };
                        header_slice[virt_offset..virt_end].copy_from_slice(&data[f_offset..f_end]);
                        byte_count = p_header.file_size() as usize;
                    }

                    info!("header_addr: {:#X?}", header_addr);
                    // Compute and write padding
                    let f_size = p_header.file_size() as usize;

                    if mem_size > f_size {
                        let pad_start = virt_offset + mem_size;
                        let pad_end = pad_start + f_size;
                        for _pad in pad_start..pad_end {
                            unsafe {
                                header_ptr.add(byte_count).write(0);
                                byte_count += 1;
                            }
                        }
                        //user_slice[pad_start..pad_end].fill(0);
                    }
                },
                _ => ()
            }
        }

        info!("Loaded elf for pid: {}", self.id);

        Ok(())
    }
}

fn entry_point(header: &Header) -> VirtAddr {
    VirtAddr::new(header.pt2.entry_point())
}
