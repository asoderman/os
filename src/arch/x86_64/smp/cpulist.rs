use core::sync::atomic::AtomicBool;

use super::Core;

use alloc::vec::Vec;
use spin::{RwLock, RwLockReadGuard};
use lazy_static::lazy_static;

static INIT: AtomicBool = AtomicBool::new(false);

lazy_static! {
    static ref CPU_LIST: RwLock<Vec<Core>> = RwLock::new(Vec::new());
}

pub fn cpu_list<'c>() -> RwLockReadGuard<'c, Vec<Core>> {
    CPU_LIST.read()
}

pub(super) fn add_core(bsp: Core) {
    if INIT.load(core::sync::atomic::Ordering::SeqCst) { panic!("Added core after cpu list is finalized"); }
    CPU_LIST.write().push(bsp);
}

/// Locks and sorts the CPU list and returns the core count
pub(super) fn finalize() -> usize {
    INIT.store(true, core::sync::atomic::Ordering::SeqCst);
    CPU_LIST.write().sort_by_key(|c| c.local_apic_id);
    CPU_LIST.read().len()
}
