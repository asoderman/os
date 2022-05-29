use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use x86_64::{PhysAddr, registers::model_specific::Msr, VirtAddr};

use crate::mm::{memory_manager, temp_page};

use super::super::pic::PICS;

use bitfield::bitfield;
use spin::once::Once;


// MSR
const X2_APIC_MSR_BASE: u32 = 0x800;
const ICR_MSR: u32 = X2_APIC_MSR_BASE + 0x30;
const SIV_MSR: u32 = X2_APIC_MSR_BASE + 0x00F;

// MMIO 
const ID_REGISTER: usize = 0x20;
const EOI_REGISTER: usize = 0xB0;
const SPURIOUS_INTERRUPT_VECTOR_REGISTER: usize = 0xF0;
const SIV_ENABLE: u32 = 0x100;
const SPURIOUS_INTERRUPT_NUM: u32 = 0xFF;
const ICR_LOW: usize = 0x300;
const ICR_HIGH: usize = 0x310;

static ENABLE_APIC_MSR: Once = Once::new();

static PIC_DISABLED: AtomicBool = AtomicBool::new(false);

/// The Lapic base read from the madt
pub static mut LAPIC_BASE: AtomicUsize = AtomicUsize::new(usize::max_value());

bitfield! {
    /// The interrupt command register for the LAPIC
    /// This component is located at base + 0x300 and must be written second.
    pub struct Icr(u32);
    impl Debug;

    /// The vector number, or starting page number for SIPIs
    pub vec, set_vec: 7, 0;
    /// The destination mode. 0 is normal, 1 is lowest priority, 2 is SMI, 4 is NMI, 5 can be INIT 
    /// or INIT level de-assert, 6 is a SIPI.
    pub dst_mode, set_dst_mode: 10, 8;
    /// The destination mode. Clear for a physical destination, or set for a logical destination. 
    /// If the bit is clear, then the destination field in 0x310 is treated normally.
    pub phys_dst_mode, set_phys_dst_mode: 11;
    /// Delivery status. Cleared when the interrupt has been accepted by the target.
    pub deliv_status, set_deliv_status: 12;
    /// Clear for INIT level de-assert, otherwise set.
    pub _14, set_14: 14;
    /// Set for INIT level de-assert, otherwise clear. 
    pub _15, set_15: 15;
    /// Destination type. If this is > 0 then the destination field in 0x310 is ignored. 1 will always 
    /// send the interrupt to itself, 2 will send it to all processors, and 3 will send it to all 
    /// processors aside from the current one. It is best to avoid using modes 1, 2 and 3, and 
    /// stick with 0.
    pub destination_type, _: 19, 18;
}

impl Icr {
    /// Creates a new empty Icr
    fn new() -> Self{
        Icr(0)
    }

    /// Creates a new level triggered, assert, init ipi
    fn init() -> Self {
        let mut i = Self::new();
        i.set_14(true);
        i.set_15(true);
        i.set_dst_mode(5);

        i
    }

    /// Creates a new level triggered, de-assert, init ipi
    fn init_deassert() -> Self {
        let mut i = Self::init();
        i.set_14(false);
        i.set_15(true);

        i
    }

    /// Creates a new edge (?) triggered, start ipi
    fn sipi(vector: u8) -> Self {
        let mut s = Self::new();

        s.set_vec(vector as u32);

        s.set_dst_mode(6);

        s
    }
}

bitfield! {
    /// The destination component of the ICR.
    /// This component is located at base + 0x310 and must be written first.
    pub struct IcrDst(u32);
    impl Debug;
    pub dst, set_dst: 27, 24;
}


pub enum Ipi {
    Init(bool),
    Sipi(u8),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Lapic {
    /// The IA32_APIC_BASE
    vaddr: VirtAddr,
    x2: bool
}

impl Drop for Lapic {
    fn drop(&mut self) {
        memory_manager().unmap_region(self.vaddr, 1).unwrap();
    }
}

impl Lapic {
    /// Create an interface to the local apic and map the registers into memory.
    pub fn new() -> Self {
        unsafe {
            let base_phys = PhysAddr::new(LAPIC_BASE.load(Ordering::SeqCst) as u64);
            if base_phys.as_u64() == u64::max_value() { panic!("LAPIC_BASE not set") }
            let vaddr = memory_manager().kmap_mmio_anywhere(base_phys, 1).expect("Could not map lapic");
            crate::println!("Mapping lapic to {:?}", vaddr);
            Self {
                vaddr,
                x2: false,
            }
        }
    }


    /// Initialize the CPU lapic. Disables interrupts on the core then disables the PIC if not
    /// already disabled. Then writes to the spurious interrupt vector to enable the LAPIC.
    pub fn initialize(&self) -> Result<(), ()> {

        crate::interrupt::disable_interrupts();

        if PIC_DISABLED.load(core::sync::atomic::Ordering::SeqCst) == false {
            disable_pic()
        }

        if self.x2 {
            ENABLE_APIC_MSR.call_once(|| {
                enable_x2apic_msr();
            });
        }

        //Write the spurious interrupt vector to enable interrupts on the LAPIC
        if self.x2 {
            self.msr_write_siv(SIV_ENABLE | SPURIOUS_INTERRUPT_NUM);
        } else {
            self.write_siv(SIV_ENABLE | SPURIOUS_INTERRUPT_NUM);
        }
        self.eoi();
        Ok(())
    }

    /// Read the lapic id register
    pub fn id(&self) -> u32 {
        unsafe {
            core::ptr::read((self.vaddr.as_u64() as usize + ID_REGISTER) as *const u32)
        }
    }

    /// Allocates a trampoline in low memory then performs the universal startup algorithm
    /// specified by Intel
    pub fn wake_core(&self, lapic_id: u32) -> Result<(), ()> {

        let trampoline_vec = super::trampoline::Trampoline::new(lapic_id);

        // Universal startup algorithm
        self.send_interrupt(Ipi::Init(true), lapic_id).unwrap();
        super::super::pit::pit().wait(200);
        self.send_interrupt(Ipi::Init(false), lapic_id).unwrap();
        super::super::pit::pit().wait(200);
        self.send_interrupt(Ipi::Sipi(trampoline_vec as u8), lapic_id).unwrap();

        crate::println!("Wake core sent");

        loop {} // TODO: REMOVE THIS

        Ok(())
    }

    /// Send an inter-processor interrupt
    pub fn send_interrupt(&self, interrupt: Ipi, lapic_id: u32) -> Result<(), ()> {
        let mut timeout: usize = 10_000_000;
        let destination = {
            let mut i = IcrDst(0);
            i.set_dst(lapic_id);
            i
        };
        let int = match interrupt {
            Ipi::Sipi(vec) => Icr::sipi(vec),
            Ipi::Init(assert) => {
                if assert { Icr::init() } else { Icr::init_deassert() }
            }
        };


        if self.x2 {
            self.msr_write_icr((int.0 as u64) | ((destination.0 as u64) << 32));
        } else {
            self.write_icr_high(destination.0);
            self.write_icr_low(int.0);
        }

        loop {
            // Delivery status will be cleared when the interrupt is delivered
            if self.read_icr_low().deliv_status() {
                timeout -= 1;
                if timeout == 0 {
                    Err(())?
                }
            } else {
                return Ok(())
            }
        }
    }

    fn write_siv(&self, val: u32) {
        let addr = self.vaddr.as_u64() as usize + SPURIOUS_INTERRUPT_VECTOR_REGISTER;
        unsafe {
            core::ptr::write(addr as *mut u32, val);
        }
    }

    fn msr_write_siv(&self, val: u32) {
        let mut msr = Msr::new(SIV_MSR);
        unsafe {
            msr.write(val as u64);
        }
    }

    #[allow(dead_code)]
    fn read_siv(&self) -> u32 {
        unsafe {
            core::ptr::read_volatile((self.vaddr.as_u64() as usize + SPURIOUS_INTERRUPT_VECTOR_REGISTER) as *const u32)
        }
    }

    fn msr_write_icr(&self, val: u64) {
        let mut msr = Msr::new(ICR_MSR);
        unsafe {
            msr.write(val);
        }
    }

    fn write_icr_low(&self, val: u32) {
        let addr = self.vaddr.as_u64() as usize + ICR_LOW;
        unsafe {
            core::ptr::write_volatile(addr as *mut u32, val);
        }
    }

    fn write_icr_high(&self, val: u32) {
        let addr = self.vaddr.as_u64() as usize + ICR_HIGH;
        unsafe {
            core::ptr::write_volatile(addr as *mut u32, val);
        }
    }

    fn read_icr_low(&self) -> Icr {
        let addr = self.vaddr.as_u64() as usize + ICR_LOW;
        unsafe {
            Icr(core::ptr::read_volatile(addr as *const u32))
        }
    }

    fn eoi(&self) {
        let addr = self.vaddr.as_u64() as usize + EOI_REGISTER;
        unsafe {
            core::ptr::write_volatile(addr as *mut u8, 0);
        }
    }
}

fn disable_pic() {
    let mut pic = PICS.lock();
    pic.disable();

    PIC_DISABLED.store(true, core::sync::atomic::Ordering::SeqCst);
}

/// Enables the x2apic which uses MSRs
fn enable_x2apic_msr() {
    let mut msr = x86_64::registers::model_specific::Msr::new(0x1B);
    unsafe {
        let old = msr.read();
        msr.write(old | (0b1 << 10));
    }
}
