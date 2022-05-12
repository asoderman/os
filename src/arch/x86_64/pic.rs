use x86_64::instructions::port::{Port};

use lazy_static::lazy_static;

use spin::Mutex;

const CH0: u16 = 0x40;
const CH1: u16 = 0x41;
const CH2: u16 = 0x42;
const MODE_REG: u16 = 0x43;

const OPERATING_MODE_SQUARE: u8 = 3;

lazy_static! {
    pub static ref PICS: Mutex<ChainedPic> = Mutex::new(ChainedPic::new(Pic::new(0x20, 0x21), Pic::new(0xA0, 0xA1)));

}


/*
struct PIC {
    channel_1: Port<u8>,
    channel_2: Port<u8>,
    channel_3: Port<u8>,
    mode: PortWriteOnly<u8>,
}
*/

pub struct ChainedPic {
    parent: Pic,
    child: Pic,
}

impl ChainedPic { 
    pub fn new(parent: Pic, child: Pic) -> ChainedPic {
        ChainedPic { parent, child }
    }

    pub fn disable(&mut self) {
        self.parent.disable();
        self.child.disable();

    }
}

pub struct Pic {
    command_port: Port<u8>,
    data_port: Port<u8>
}

impl Pic {
    fn new(command_port: u16, data_port: u16) -> Self {
        Pic {
            command_port: Port::new(command_port),
            data_port: Port::new(data_port),
        }
    }

    pub fn disable(&mut self) {
        unsafe { self.data_port.write(0xFFu8) }
    }
}
