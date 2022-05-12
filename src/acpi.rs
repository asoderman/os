use acpi::{AcpiHandler, PhysicalMapping, AcpiTables, PlatformInfo};


use libkloader::KernelInfo;
use x86_64::{align_up, align_down};

use crate::mm::{memory_manager};

use crate::arch::{PhysAddr, VirtAddr, PAGE_SIZE};

use core::ptr::NonNull;

#[derive(Debug, Clone)]
pub struct AcpiMapper(usize);

impl AcpiHandler for AcpiMapper {
    // FIXME: just identity map this
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> PhysicalMapping<Self, T> {
        let start = PhysAddr::new(align_down(physical_address as u64, PAGE_SIZE as u64));
        let end = PhysAddr::new(align_up((physical_address + size) as u64, PAGE_SIZE as u64));
        let frame_count = (end - start) / PAGE_SIZE as u64;

        memory_manager().map_unchecked(start, frame_count as usize).expect("Error identity mapping with ACPI");

        let v_start = VirtAddr::new(physical_address as u64);
        PhysicalMapping::new(physical_address, NonNull::new_unchecked(v_start.as_mut_ptr() as *mut T), size, frame_count as usize, self.clone())
    }

    fn unmap_physical_region<T>(region: &PhysicalMapping<Self, T>) {
        for frame in 0..region.mapped_length() {
            let addr = region.physical_start() + frame * PAGE_SIZE;
            memory_manager().kunmap_untracked(VirtAddr::new(addr as u64));
        }
    }
}

pub fn acpi_tables(bootinfo: &KernelInfo) -> AcpiTables<AcpiMapper> {
    unsafe {
        AcpiTables::from_rsdp(AcpiMapper(0), bootinfo.acpi_info.rsdp_base as usize).unwrap()
    }
}

pub fn platform_info(tables: AcpiTables<AcpiMapper>) -> PlatformInfo {
    tables.platform_info().unwrap()
}

pub fn processors(info: PlatformInfo) -> usize {
    let p = info.processor_info.unwrap();

    1 + p.application_processors.len()
}
