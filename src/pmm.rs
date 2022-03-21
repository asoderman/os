use crate::arch::{VirtAddr, PhysAddr, PAGE_SIZE};
use crate::dev::serial::write_serial_out;
use libkloader::MemoryMapInfo;
use libkloader::{uefi::MemoryType, MemoryDescriptor};

use x86_64::structures::paging::frame::{PhysFrame, PhysFrameRange};
use x86_64::structures::paging::page::Size4KiB;

#[derive(Debug)]
pub struct PhysicalMemoryManager {

}

impl PhysicalMemoryManager {
    fn new(heap_start: VirtAddr, heap_end: VirtAddr, phys_offset: usize) {

    }
}

pub fn init_pmm(heap_range: (VirtAddr, VirtAddr)) {

}

pub fn get_init_heap_section(
    size: usize,
    mem_map: MemoryMapInfo,
) -> Result<PhysFrameRange<Size4KiB>, &'static str> {
    let mut sect = None;

    let mut reserved_count = 0;

    let mut index = 0;

    write_serial_out("Mem type search begin \n");
    /*
    for descriptor in mem_map.iter() {
        match descriptor.ty {
                7 => {
                write_serial_out("Mem type match \n");
                if descriptor.page_count >= size as u64 {
                    sect = Some(descriptor.phys_start);
                    break;
                }
            }
            0 => { reserved_count += 1; },
            _ => ()
        }
    }
    */

    while index < mem_map.count {
        let descriptor_opt = mem_map.get(index);
        if descriptor_opt.is_none() {
            write_serial_out("descriptor is non\n");
        }

        write_serial_out("Candidate descriptor deref\n");
        let descriptor = descriptor_opt.unwrap();
        match descriptor.ty {
            7 => {
                write_serial_out("Mem type match \n");
                if descriptor.page_count >= size as u64 {
                    sect = Some(descriptor.phys_start);
                    break;
                }
            }
            0 => {
                reserved_count += 1;
            }
            _ => (),
        }
        index += 1;
    }

    write_serial_out("Mem search done \n");
    if reserved_count == mem_map.count {
        return Err("<ERROR> Entire mem map reserved. Likely corrupted");
    }
    let start_address = PhysAddr::new(sect.ok_or("<ERROR>No suitable memory start_addr found\n")?);
    let start_frame = PhysFrame::containing_address(start_address);
    //.map_err(|_| "Start Address unaligned")?;
    let end_frame = PhysFrame::containing_address(start_address + (size as u64 * PAGE_SIZE as u64));
    //.map_err(|_| "End Address unaligned")?;

    Ok(PhysFrame::range(start_frame, end_frame))
}
