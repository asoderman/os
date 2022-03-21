
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        use crate::dev::serial::SERIAL_OUT;
        SERIAL_OUT.lock().write_fmt(format_args!($($arg)*)).unwrap();
        $crate::print!("\n");
    })
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        use crate::dev::serial::SERIAL_OUT;
        SERIAL_OUT.lock().write_fmt(format_args!($($arg)*)).unwrap();
    })
}
