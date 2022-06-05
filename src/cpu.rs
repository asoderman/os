use core::fmt::Debug;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq)]
#[repr(transparent)]
pub struct CoreId(pub usize);

pub fn current_id() -> CoreId {
    let lapic_id = lapic_id();
    CoreId(lapic_id as usize)
}

fn lapic_id() -> u32 {
    crate::arch::x86_64::apic_id() as u32
}

fn core_to_lapic(core_id: CoreId) -> Option<u32> {
    crate::arch::x86_64::smp::cpu_list().get(core_id.0).map(|cpu| cpu.read().local_apic_id)
}

