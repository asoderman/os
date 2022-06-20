use x86_64::instructions::port::Port;

use lazy_static::lazy_static;

use spin::Mutex;

lazy_static! {
    pub static ref PICS: Mutex<ChainedPic> = Mutex::new(ChainedPic::new(Pic::new(0x20, 0x21), Pic::new(0xA0, 0xA1)));
}

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
    _command_port: Port<u8>,
    data_port: Port<u8>
}

impl Pic {
    fn new(command_port: u16, data_port: u16) -> Self {
        Pic {
            _command_port: Port::new(command_port),
            data_port: Port::new(data_port),
        }
    }

    pub fn disable(&mut self) {
        unsafe { self.data_port.write(0xFFu8) }
    }
}
