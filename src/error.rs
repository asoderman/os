use core::fmt::{Debug};

use alloc::boxed::Box;

pub trait Error: Debug {
    fn source(&self) -> Option<&Box<dyn Error>>;

}
