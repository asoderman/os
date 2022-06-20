use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::vec::Vec;
use libkloader::KernelInfo;
use ::acpi::{InterruptModel, platform::ProcessorInfo};
use spin::{RwLockReadGuard, RwLockWriteGuard};
use x86_64::PhysAddr;
use crate::acpi;

use self::lapic::Lapic;

mod cpulist;
pub mod lapic;
mod trampoline;
pub(super) mod thread_local;

pub use cpulist::cpu_list;

static CORES: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Default)]
pub struct CpuLocals {
    pub lapic_id: usize,
    is_init: bool
}

impl CpuLocals {
    const fn empty() -> Self {
        CpuLocals {
            lapic_id: usize::max_value(),
            is_init: false,
        }
    }

    pub(super) fn init(lapic_id: usize) {
        unsafe {
            CPU_LOCALS.lapic_id = lapic_id;
            CPU_LOCALS.is_init = true;
        }
    }

    pub fn get<'local>() -> &'local Self {
        unsafe {
            assert!(CPU_LOCALS.is_init);
            &CPU_LOCALS
        }
    }

    pub fn try_get<'local>() -> Option<&'local Self> {
        // TODO: gsbase!
        let tl_base = x86_64::registers::model_specific::FsBase::read();

        if tl_base == super::VirtAddr::new(0u64) {
            None
        } else {
            unsafe {
                CPU_LOCALS.is_init.then_some(&CPU_LOCALS)
            }
        }
    }
}

#[thread_local]
static mut CPU_LOCALS: CpuLocals = CpuLocals::empty();

#[macro_export]
macro_rules! core {
    () => {{
        use crate::arch::x86_64::smp::cpu_list;
        use crate::arch::x86_64::apic_id;
        cpu_list().get(apic_id() as usize).expect("Could not get current core using core!")
    }};
    ($id:expr) => {{
        use crate::arch::x86_64::smp::cpu_list;
        cpu_list().get($id).expect("Could not get current core using core!")
    }}
}

/// Returns the BSP
macro_rules! bsp { 
    () => {{
        core!(0)
    }}
}

#[derive(Default, Debug)]
pub struct Core {
    processor_uid: u32,
    pub local_apic_id: u32,

    is_ap: bool,
    /// The ready flag indicates the Core is ready and waiting for the scheduler
    ready: bool,
}

impl Core {
    fn new(local_apic_id: u32) -> Self {
        Self {
            local_apic_id,
            ..Self::default()
        }
    }
}

#[derive(Debug)]
pub enum SmpError {
    UnknownInterruptModel,
}

/// Initializes the SMP subsystem
pub fn init_smp() -> Result<(), SmpError> {
    crate::println!("Enabling SMP");
    let tables = acpi::acpi_tables();
    let info = acpi::platform_info(tables);

    match info.interrupt_model {
        InterruptModel::Apic(apic) => {
            self::lapic::set_base(PhysAddr::new(apic.local_apic_address));

            apic_list_cores(info.processor_info.as_ref().unwrap());

            thread_local::init_thread_local(bsp!().read().local_apic_id as usize);
        },
        _ => {
            return Err(SmpError::UnknownInterruptModel);
        },
    };

    let lapic = Lapic::new();
    // initialize bsp lapic
    lapic.initialize().unwrap();

    let mut trampoline = trampoline::Trampoline::new();

    for core in cpu_list().iter().map(|core| core.read()).filter(|core| core.is_ap) {
        // wake smp cores
        lapic.wake_core(core.local_apic_id, &mut trampoline).unwrap();
    }

    trampoline.destroy().unwrap();

    Ok(())
}

/// Constructs the global list of CPU cores.
fn apic_list_cores(info: &ProcessorInfo) {
    let mut cores = Vec::new();
    let mut bsp = Core::new(info.boot_processor.local_apic_id);
    bsp.is_ap = false;
    bsp.processor_uid = info.boot_processor.processor_uid;
    bsp.ready = true;

    cores.push(bsp);

    for p in info.application_processors.iter().enumerate() {
        let mut ap = Core::new(p.1.local_apic_id);
        ap.is_ap = true;
        ap.processor_uid = p.1.processor_uid;
        cores.push(ap);
    }

    let core_count = cpulist::init_cpu_list(cores);

    CORES.store(core_count, Ordering::SeqCst);
}

pub(super) fn init_smp_ap(lapic_id: usize) {
    thread_local::init_thread_local(lapic_id);
    CpuLocals::init(lapic_id);
    lapic::Lapic::new().initialize().expect("Failed to initialize LAPIC for ap!");
}

/// Gets the lapic for the current core
fn lapic() -> Lapic {
    Lapic::new()
}

pub fn this_core<'c>() -> RwLockReadGuard<'c, Core> {
    cpu_list()[CpuLocals::get().lapic_id].read()
}

pub fn this_core_mut<'c>() -> RwLockWriteGuard<'c, Core> {
    cpu_list()[CpuLocals::get().lapic_id].write()
}

/// Gets the smp core count
pub fn smp_cores() -> usize {
    CORES.load(Ordering::SeqCst)
}
