use core::sync::atomic::{AtomicUsize, Ordering};

use super::smp::lapic::Lapic;


static mut LAPIC_TICKS_PER_10_MS: AtomicUsize = AtomicUsize::new(0);


pub struct LapicTimer<'lapic> {
    lapic: &'lapic Lapic,
}

impl<'lapic> LapicTimer<'lapic> {

    pub fn new(lapic: &'lapic Lapic) -> Self {
        LapicTimer {
            lapic,
        }
    }

    pub fn set_interrupt_number(&self, value: u16) {

    }

    /// Enables the lapic timer
    pub(super) fn enable(&self) {
        self.lapic.unmask_timer();
    }

    pub(super) fn disable(&self) {
        self.lapic.mask_timer();
    }

    pub(super) fn calibrate(&self) -> Result<(), ()> {
        const START_COUNT: u32 = 0xFFFFFFFF;
        // Tell apic timer to use divider 16
        self.lapic.write_apic_timer_div(0x3);
        // write -1 to the apic timer inital count
        self.lapic.write_apic_register_initcnt(START_COUNT);

        // start timer
        self.enable();

        super::pit::pit().wait(10);

        // stop timer
        self.disable();

        let ticks_per_10_ms = START_COUNT - self.lapic.read_apic_timer_current_count();

        unsafe {
            LAPIC_TICKS_PER_10_MS.store(ticks_per_10_ms as usize, Ordering::SeqCst);
        }

        crate::println!("ticks per 10 ms: {:?}", ticks_per_10_ms);

        Ok(())
    }

}

