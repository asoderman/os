use core::sync::atomic::{AtomicBool, Ordering};

use alloc::{collections::BTreeMap, vec::Vec};
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};

use crate::{mm::{memory_manager, temp_page, TempPageGuard}, arch::PAGE_SIZE};

use lazy_static::lazy_static;

const TRAMPOLINE_LOAD_ADDRESS: u64 = 0x8000;

lazy_static! {
    static ref SMP_CORES_READY: Vec<AtomicBool> = {
        let mut v = Vec::new();
        let core_count = super::smp_cores();
        // Mark the bsp as ready
        v.push(AtomicBool::new(true));
        for _ in 1..core_count {
            v.push(AtomicBool::new(false));
        }
        v
    };
}

static TRAMPOLINE_CODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/trampoline"));

#[derive(Debug, Default, Clone, Copy)]
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
    //_frame_guard: TempPageGuard,
    args: TrampolineArgs,
    vector: usize
}

impl Trampoline {
    /// Creates a new trampoline to manage the resources to boot an ap.
    ///
    /// # Returns
    /// The destination vector number
    pub fn new() -> Trampoline {
        crate::println!("Creating Trampoline");

        let phys_frame = Self::allocate_low_frame();
        //let frame_guard = temp_page(VirtAddr::new(phys_frame.as_u64()));

        crate::println!("Trampoline created");

        Self {
            phys_frame,
            //_frame_guard: frame_guard,
            args: TrampolineArgs::default(),
            vector: phys_frame.as_u64() as usize / PAGE_SIZE,
        }
    }

    /// Allocate a physical frame < (255 * Page Size). This is necessary because the ap starts on
    /// the page specified by a 1 byte start vector.
    fn allocate_low_frame() -> PhysAddr {
        let addr = PhysAddr::new(TRAMPOLINE_LOAD_ADDRESS);
        memory_manager().k_identity_map(addr, 1);
        addr
    }

    /// Returns the vector i.e. the frame number where the trampoline is located
    pub fn vector(&self) -> usize {
        self.vector
    }

    /// Write the trapoline code from trampoline.S to the vector
    unsafe fn write_trampoline(&self) {
        crate::mm::write_physical_slice(self.phys_frame, TRAMPOLINE_CODE);
    }

    /// Writes extra data needed to get ap to long mode
    unsafe fn write_trampoline_args(&self) {
        crate::mm::write_physical(self.phys_frame + 8u64, self.args);
    }

    pub fn configure(&mut self, lapic_id: usize) {
        let page_table = x86_64::registers::control::Cr3::read().0.start_address();

        let ap_rsp = crate::stack::allocate_kernel_stack();
        crate::println!("new Kernel stack created");

        self.args = TrampolineArgs {
            page_table: page_table.as_u64() as usize,
            rsp: ap_rsp.as_u64() as usize,
            ap_entry: ap_entry as usize,
            lapic_id: lapic_id as usize
        };

        unsafe {
            self.write_trampoline();
            // Write the args 8 bytes after the start of the trampoline
            self.write_trampoline_args();
        }
    }

    pub fn destroy(self) -> Result<(), ()> {
        memory_manager().kunmap(VirtAddr::new(self.phys_frame.as_u64())).map_err(|_| ())
    }
}

pub(super) fn wait_for_core(lapic_id: usize) {
    while !SMP_CORES_READY.get(lapic_id).expect("Invalid lapic id").load(Ordering::SeqCst) {}
}

/// The rust entry point for ap's
pub extern "C" fn ap_entry(lapic_id: usize) {
    crate::println!("ap_entry lapic_id: {}", lapic_id);
    crate::arch::x86_64::ap_init(lapic_id);
    SMP_CORES_READY[super::this_core().local_apic_id as usize].store(true, Ordering::SeqCst);

    for (i, core) in SMP_CORES_READY.iter().enumerate() {
        crate::println!("core {} ready:{:?}", i, core);
    }

    // Park the ap until scheduler is ready
    crate::ap_main();
}
