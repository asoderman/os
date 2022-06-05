use super::Core;

use alloc::vec::Vec;
use spin::RwLock;
use spin::Once;

static CPU_LIST: Once<Vec<RwLock<Core>>> = Once::new();

pub fn cpu_list<'c>() -> &'c [RwLock<Core>] {
    CPU_LIST.get().expect("Attempted to get cpu list before initialization").as_slice()
}

pub(super) fn init_cpu_list(cores: Vec<Core>) -> usize {
    CPU_LIST.call_once(move || {
        let mut cores = cores;
        cores.sort_by_key(|c| c.local_apic_id);
        let list = cores.into_iter().map(|c| RwLock::new(c)).collect();
        list
    });
    unsafe {
        CPU_LIST.get_unchecked().len()
    }
}

