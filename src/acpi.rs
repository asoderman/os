use acpi::{AcpiHandler, PhysicalMapping, AcpiTables, PlatformInfo};

use x86_64::{align_up, align_down};

use crate::mm::memory_manager;
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

        memory_manager().kmap_identity_mmio(start, frame_count as usize).expect("Error identity mapping with ACPI");

        let v_start = VirtAddr::new(physical_address as u64);
        PhysicalMapping::new(physical_address, NonNull::new_unchecked(v_start.as_mut_ptr() as *mut T), size, frame_count as usize, self.clone())
    }

    fn unmap_physical_region<T>(region: &PhysicalMapping<Self, T>) {
        memory_manager().unmap_region(VirtAddr::new(region.virtual_start().as_ptr() as u64), region.mapped_length()).unwrap();
    }
}

pub fn acpi_tables() -> AcpiTables<AcpiMapper> {
    let rsdp_base = crate::env::env().rsdp_base;
    unsafe {
        AcpiTables::from_rsdp(AcpiMapper(0), rsdp_base).unwrap()
    }
}

pub fn platform_info(tables: AcpiTables<AcpiMapper>) -> PlatformInfo {
    tables.platform_info().unwrap()
}
