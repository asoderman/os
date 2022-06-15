use core::ptr::addr_of;
use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};

use x86_64::registers::model_specific::Msr;
use x86_64::structures::tss::TaskStateSegment;

use crate::arch::PAGE_SIZE;

/// What BSP's fs is set to. Used to calculate each TLS entry
static mut BSP_TLS_BASE: AtomicUsize = AtomicUsize::new(0);
static mut TLS_SIZE: AtomicUsize = AtomicUsize::new(0);
static mut TLS_INIT: AtomicBool = AtomicBool::new(false);

const FS_BASE_MSR: u32 = 0xC0000100;
// TODO: Use gsbase for kernel thread_locals once supported
const GS_BASE_MSR: u32 = 0xC0000101;
const KERNEL_GS_BASE_MSR: u32 = 0xC0000102;

/// Writes to the fs_base MSR
unsafe fn set_fs_base(addr: usize) {
    crate::println!("set fs base: {:X}", addr);
    Msr::new(FS_BASE_MSR).write(addr as u64);
}

unsafe fn set_kernel_gs_base(addr: usize) {
    Msr::new(GS_BASE_MSR).write(addr as u64);
}

#[repr(C)]
pub struct ProcessorControlBlock {
    pub tls_self_ptr: usize,
    pub tmp_user_rsp: usize,
    pub tss: TaskStateSegment
}

impl ProcessorControlBlock {
    /// Gets the thread local ProcessorControlBlock
    pub fn get() -> &'static mut Self {
        unsafe {
            let fs = Msr::new(FS_BASE_MSR).read();
            (fs as *mut Self).as_mut().expect("Tried to access ProcessorControlBlock before thread local init")
        }
    }
}

// Exposed in the linker script
extern "C" {
    static mut __tdata_start: u8;
    static mut __tdata_end: u8;
    static mut __tbss_start: u8;
    static mut __tbss_end: u8;
}

/// Calculates the size of TLS and sets fsbase to the offset within the TLS 
pub fn init_thread_local(lapic_id: usize) {
    // TODO: This function should verify there is enough memory mapped to accomodate x cores since
    // _tdata -> tbss is only the size for a single core
    unsafe {
        // Initialize TCB once
        init_tcb(super::CORES.load(Ordering::SeqCst));

        // get tls base for our lapic id
        set_fs_base(tls_base(lapic_id));
        set_kernel_gs_base(tls_base(lapic_id));
    }
}

/// Configures the TLS. Computes all the base addresses and writes the base to itself. Also copies
/// any initialization data for each core
fn init_tcb(cores: usize) {
    unsafe {
        // If we already configured TLS leave
        if TLS_INIT.load(Ordering::SeqCst) { return; }

        // Size of uninitialized data
        let tbss_size = addr_of!(__tbss_end) as usize - addr_of!(__tbss_start) as usize;
        // Size we need to copy
        let tls_image_size = addr_of!(__tdata_end) as usize - addr_of!(__tdata_start) as usize;
        // Align the size offset to 8 bytes otherwise need natural alignment
        let tls_size = {
            let size = tls_image_size + tbss_size;
            let alignment = (size as *const u8).align_offset(core::mem::align_of::<usize>());
            size + alignment
        };
        TLS_SIZE.store(tls_size, Ordering::SeqCst);

        assert!(tls_size * cores < PAGE_SIZE, "TODO: Implement TLS for > PAGE_SIZE");

        // Create a slice of the data we need to copy
        let tls_image = core::slice::from_raw_parts(addr_of!(__tdata_start), tls_image_size);
        // Compute the first base and write the address to itself per SysV abi
        let start = addr_of!(__tdata_start).add(tls_size);
        (start as *mut usize).write(start as usize);
        BSP_TLS_BASE.store(start as usize, Ordering::SeqCst);

        // start at 1 since we pulled out the first iter
        for c in 1..=cores {
            // Compute next base and next data start ptr
            let next_base = start.add(c * (core::mem::size_of::<ProcessorControlBlock>() + tls_size));
            // Subtract the aligned tls size from the base. This is how the cpu computes this via
            // negative offsets
            let next_data = next_base.sub(tls_size);
            // Write base to itself
            (next_base as *mut usize).write(next_base as usize);
            // Copy the TLS image 
            core::ptr::copy_nonoverlapping(tls_image.as_ptr(), next_data as *mut u8, tls_image_size);
        }

        // Run once
        TLS_INIT.store(true, Ordering::SeqCst);
    }
}

/// Calculates the fs base for thread locals based on the bsp's tls. Takes the index e.g. the lapic
/// id as arg
fn tls_base(index: usize) -> usize {
    unsafe {
        let first = BSP_TLS_BASE.load(Ordering::SeqCst);
        let offset = (TLS_SIZE.load(Ordering::SeqCst) + core::mem::size_of::<ProcessorControlBlock>()) * index;
        first + offset
    }
}
