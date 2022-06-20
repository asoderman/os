use super::smp::lapic::Lapic;

pub struct LapicTimer<'lapic> {
    lapic: &'lapic Lapic,
    ticks_per_10_ms: usize,
}

impl<'lapic> LapicTimer<'lapic> {

    pub fn new(lapic: &'lapic Lapic) -> Self {
        LapicTimer {
            lapic,
            ticks_per_10_ms: 0,
        }
    }

    pub fn set_interrupt_number(&self, value: u8) {
        let reg = self.lapic.read_lvt_timer_reg() & 0xFFFFF00;
        self.lapic.write_apic_lvt_tmr(reg | value as u32);
    }

    pub(super) fn set_periodic_mode(&self) {
        let reg = self.lapic.read_lvt_timer_reg() & 0xFFFFFFF;
        // https://wiki.osdev.org/APIC_timer
        self.lapic.write_apic_lvt_tmr(reg | 0x20000);
    }

    /// Enables the lapic timer
    pub(super) fn enable(&self) {
        self.lapic.unmask_timer();
    }

    pub(super) fn disable(&self) {
        self.lapic.mask_timer();
    }

    pub(super) fn calibrate(&mut self) -> Result<(), ()> {
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

        self.ticks_per_10_ms = ticks_per_10_ms as usize;

        self.lapic.write_apic_register_initcnt(self.ticks_per_10_ms as u32);
        println!("ticks per 10 ms: {:?}", ticks_per_10_ms);

        Ok(())
    }

}

