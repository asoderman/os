use core::sync::atomic::{AtomicUsize, Ordering};

use libkloader::KernelInfo;
use ::acpi::{InterruptModel, platform::ProcessorInfo};
use crate::acpi;

use self::lapic::Lapic;

mod cpulist;
pub mod lapic;
mod trampoline;
pub(super) mod thread_local;

pub use cpulist::cpu_list;

static CORES: AtomicUsize = AtomicUsize::new(0);

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
pub fn init_smp(bootinfo: &KernelInfo) -> Result<(), SmpError> {
    crate::println!("Enabling SMP");
    let tables = acpi::acpi_tables(bootinfo);
    let info = acpi::platform_info(tables);

    match info.interrupt_model {
        InterruptModel::Apic(apic) => {
            // TODO: put this in a function
            unsafe {
                self::lapic::LAPIC_BASE.store(apic.local_apic_address as usize, Ordering::SeqCst);
            }
            apic_list_cores(info.processor_info.as_ref().unwrap());
            thread_local::init_thread_local(bsp!().local_apic_id as usize);
        },
        _ => {
            return Err(SmpError::UnknownInterruptModel);
        },
    };

    let lapic = Lapic::new();
    lapic.initialize().unwrap();
    lapic.wake_core(1).unwrap();

    Ok(())
}

/// Constructs the global list of CPU cores.
fn apic_list_cores(info: &ProcessorInfo) {
    let mut bsp = Core::new(0);
    bsp.is_ap = false;
    bsp.processor_uid = info.boot_processor.processor_uid;
    bsp.local_apic_id = info.boot_processor.local_apic_id;

    cpulist::add_core(bsp);

    for p in info.application_processors.iter().enumerate() {
        let mut ap = Core::new(p.1.local_apic_id);
        ap.is_ap = true;
        ap.processor_uid = p.1.processor_uid;
        ap.local_apic_id = p.1.local_apic_id;
        cpulist::add_core(ap);
    }

    CORES.store(cpulist::finalize(), Ordering::SeqCst);
}

/// Gets the lapic for the current core
fn lapic() -> Lapic {
    Lapic::new()
}

/// Gets the global cpu list which contains read only info about the other CPUs

/// Gets the smp core count
pub fn smp_cores() -> usize {
    CORES.load(Ordering::SeqCst)
}
