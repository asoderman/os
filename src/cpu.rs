use core::cell::Cell;
use core::sync::atomic::AtomicUsize;
use core::fmt::Debug;

use crate::acpi;
use crate::arch::x86_64::cpu::Lapic;
use crate::arch::PhysAddr;

use ::acpi::{InterruptModel, platform::ProcessorInfo};
use alloc::collections::BTreeMap;
use libkloader::KernelInfo;
use lazy_static::lazy_static;
use spin::{Mutex, MutexGuard, RwLock, RwLockReadGuard};

const BSP_CORE_ID: usize = 0;

static mut CORES: AtomicUsize = AtomicUsize::new(0);

lazy_static! {
    static ref CPU_LIST: RwLock<BTreeMap<CoreId, Cpu>> = RwLock::new(BTreeMap::new());
    static ref LAPIC_ID_LUT: RwLock<BTreeMap<u32, CoreId>> = RwLock::new(BTreeMap::new());
}

#[macro_export]
macro_rules! core {
    () => {{
        use crate::cpu::{cpu_list, current_id};
        cpu_list().get(&current_id()).expect("Could not get current core using core!")
    }};
    ($id:expr) => {{
        use crate::cpu::{cpu_list, CoreId};
        cpu_list().get(&CoreId($id)).expect("Could not get current core using core!")
    }}
}

/// Returns the BSP
macro_rules! bsp { 
    () => {{
        use crate::cpu::cpu_list;
        cpu_list().get(&CoreId(0)).unwrap()
    }}
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq)]
#[repr(transparent)]
pub struct CoreId(pub usize);

pub enum CpuError {
    UnknownInterruptModel,
}

#[derive(Default)]
pub struct Cpu {
    /// The core number assigned by the Kernel
    core_id: usize,
    processor_uid: u32,
    pub local_apic_id: u32,
    lapic: Mutex<Cell<Lapic>>,

    is_ap: bool,
}

impl Debug for Cpu {
     fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Cpu: {}: {{ lapic_id: {:X} }}", self.core_id, self.local_apic_id)
    }
}

impl Ord for Cpu {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.core_id.cmp(&other.core_id)
    }
}

impl PartialOrd for Cpu {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Cpu {
    fn eq(&self, other: &Self) -> bool {
        self.core_id == other.core_id
    }
}

impl Eq for Cpu {}

impl Cpu {
    fn new(core_id: usize) -> Self {
        Self {
            core_id,
            ..Self::default()
        }
    }

    /// Locks the CPUs lapic interface. This should only be called from the core that owns the
    /// Lapic i.e. using core!
    pub fn lapic<'l>(&'l self) -> MutexGuard<'l, Cell<Lapic>> {
        self.lapic.lock()
    }
}

/// Initializes the SMP subsystem
pub fn init_smp(bootinfo: &KernelInfo) -> Result<(), CpuError> {
    crate::println!("Enabling SMP");
    let tables = acpi::acpi_tables(bootinfo);
    let info = acpi::platform_info(tables);

    match info.interrupt_model {
        InterruptModel::Apic(apic) => {
            // TODO: put this in a function
            unsafe {
                crate::arch::x86_64::cpu::LAPIC_BASE.store(apic.local_apic_address as usize, core::sync::atomic::Ordering::SeqCst);
            }
            apic_list_cores(info.processor_info.as_ref().unwrap());
            bsp!().lapic().set(Lapic::new(PhysAddr::new(apic.local_apic_address)));
            crate::println!("lapic id: {}", lapic_id());
            bsp!().lapic().get_mut().initialize();
            bsp!().lapic().get_mut().wake_core(1);
        },
        _ => {
            return Err(CpuError::UnknownInterruptModel);
        },
    };

    Ok(())
}

/// Constructs the global list of CPU cores.
fn apic_list_cores(info: &ProcessorInfo) {
    let mut bsp = Cpu::new(0);
    bsp.is_ap = false;
    bsp.processor_uid = info.boot_processor.processor_uid;
    bsp.local_apic_id = info.boot_processor.local_apic_id;

    LAPIC_ID_LUT.write().insert(bsp.local_apic_id, CoreId(bsp.core_id));
    CPU_LIST.write().insert(CoreId(BSP_CORE_ID), bsp);

    for p in info.application_processors.iter().enumerate() {
        let mut ap = Cpu::new(p.0 + 1);
        ap.is_ap = true;
        ap.processor_uid = p.1.processor_uid;
        ap.local_apic_id = p.1.local_apic_id;
        LAPIC_ID_LUT.write().insert(ap.local_apic_id, CoreId(ap.core_id));
        CPU_LIST.write().insert(CoreId(ap.core_id), ap);
    }
}

pub fn cpu_list<'c>() -> RwLockReadGuard<'c, BTreeMap<CoreId, Cpu>> {
    CPU_LIST.read()
}

pub fn current_id() -> CoreId {
    let lapic_id = lapic_id();
    lapic_to_core(lapic_id).unwrap_or_else(|| {
        panic!("lapic id {} did not match a core", lapic_id);
    })
}

pub fn cores() -> usize {
    let value = unsafe { CORES.load(core::sync::atomic::Ordering::Relaxed) };
    if  value == 0 {
        let count = CPU_LIST.read().len();
        unsafe {
            CORES.store(count, core::sync::atomic::Ordering::SeqCst);
        }
        count
    } else {
        value
    }
}

fn lapic_id() -> u32 {
    crate::arch::x86_64::apic_id() as u32
}

fn lapic_to_core(lapic_id: u32) -> Option<CoreId> {
    LAPIC_ID_LUT.read().get(&lapic_id).cloned()
}

fn core_to_lapic(core_id: CoreId) -> Option<u32> {
    cpu_list().get(&core_id).map(|cpu| cpu.local_apic_id)
}

