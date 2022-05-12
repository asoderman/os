use core::{arch::asm, sync::atomic::{AtomicBool, Ordering}};

use alloc::{collections::BTreeMap, vec::Vec};
use spin::Mutex;
use x86_64::{structures::{paging::PageTable, DescriptorTablePointer}, PhysAddr, VirtAddr};

use crate::{mm::{memory_manager, temp_page, TempPageGuard}, arch::PAGE_SIZE};

use lazy_static::lazy_static;

static mut ACTIVE_TRAMPOLINES: Option<BTreeMap<u32, Mutex<Trampoline>>> = None;

lazy_static! {
    static ref SMP_CORES_READY: Vec<AtomicBool> = {
        let mut v = Vec::new();
        let core_count = crate::cpu::cores();
        // Mark the bsp as ready
        v.push(AtomicBool::new(true));
        for _ in 1..core_count {
            v.push(AtomicBool::new(false));
        }
        v
    };
}

static TRAMPOLINE_CODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/trampoline"));

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct TrampolineArgs {
    page_table: usize,
    rsp: usize,
    ap_entry: usize,
    lapic_id: usize,
}

#[derive(Debug)]
pub(super) struct Trampoline {
    phys_frame: PhysAddr, // TODO: Maybe hold a temp map guard here
    _frame_guard: TempPageGuard,
    lapic_id: u32,
    args: TrampolineArgs
}

impl Trampoline {
    /// Creates a new trampoline to manage the resources to boot an ap.
    ///
    /// # Returns
    /// The destination vector number
    pub fn new(lapic_id: u32) -> usize {
        crate::println!("Creating Trampoline");

        let phys_frame = Self::allocate_low_frame();
        let frame_guard = temp_page(VirtAddr::new(phys_frame.as_u64()));

        let page_table = x86_64::registers::control::Cr3::read().0.start_address();

        let ap_rsp = crate::stack::allocate_kernel_stack();
        crate::println!("new Kernel stack created");

        let args = TrampolineArgs {
            page_table: page_table.as_u64() as usize,
            rsp: ap_rsp.as_u64() as usize,
            ap_entry: ap_entry as usize,
            lapic_id: lapic_id as usize
        };

        unsafe {
            Self::write_trampoline(phys_frame);
            Self::write_trampoline_args(phys_frame, args);
        }

        Self {
            phys_frame,
            _frame_guard: frame_guard,
            lapic_id,
            args
        }.make_active();

        crate::println!("Trampoline created");
        // return the start vector
        phys_frame.as_u64() as usize / 0x1000
    }

    /// Allocate a physical frame < (255 * Page Size). This is necessary because the ap starts on
    /// the page specified by a 1 byte start vector.
    fn allocate_low_frame() -> PhysAddr {
        let addr = PhysAddr::new(0x8000);
        memory_manager().k_identity_map(addr, 1).unwrap();
        addr
    }
    /// Write the trapoline code from trampoline.S to the vector
    unsafe fn write_trampoline(addr: PhysAddr) {
        let ptr = core::slice::from_raw_parts_mut(addr.as_u64() as usize as *mut u8, PAGE_SIZE);

        ptr[0..TRAMPOLINE_CODE.len()].clone_from_slice(TRAMPOLINE_CODE);
    }

    /// Writes extra data needed to get ap to long mode
    unsafe fn write_trampoline_args(trampoline_frame: PhysAddr, args: TrampolineArgs) {
        const ARGS_OFFSET: usize = 1;
        let ptr = (trampoline_frame.as_u64() as usize as *mut usize).add(ARGS_OFFSET) as *mut TrampolineArgs;
            ptr.write(args)
    }

    /// Insert the Trampoline into the global list to be retrieved by its core later on for cleanup
    fn make_active(self) {
        unsafe {
            match ACTIVE_TRAMPOLINES {
                None => ACTIVE_TRAMPOLINES = Some(BTreeMap::new()),
                _ => ()
            }
            ACTIVE_TRAMPOLINES.as_mut().unwrap().insert(self.lapic_id, Mutex::new(self));
        }
    }
}

/// Unmaps the trampoline page
///
/// # Safety
/// The caller must make sure all aps have been booted before destroying the trampoline
unsafe fn cleanup_trampoline(_lapic_id: u32) {
    ACTIVE_TRAMPOLINES = None;
}

/// The rust entry point for ap's
pub extern "C" fn ap_entry(lapic_id: usize) {
    crate::println!("ap_entry lapic_id: {}", lapic_id);
    SMP_CORES_READY[crate::core!(lapic_id).local_apic_id as usize].store(true, Ordering::SeqCst);
    crate::println!("cpu: {:?}", crate::core!(lapic_id));
    unsafe {
        super::super::gdt::load_kernel_gdt();
        cleanup_trampoline(lapic_id as u32);
    }
    for (i, core) in SMP_CORES_READY.iter().enumerate() {
        crate::println!("core {} ready:{:?}", i, core);
    }
    todo!("Finish initializing the ap");
    todo!("load idt");
    todo!("init lapic");
    loop {}
}
