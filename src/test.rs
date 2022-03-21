use crate::{println, print};

#[cfg(test)]
pub trait Test {
    fn run(&self) -> ();
}
#[cfg(test)]
impl<T> Test for T
where
    T: Fn()
{
    fn run(&self) {
        print!("{}...\t", core::any::type_name::<T>());
        self();
        println!("[ok]");
    }
}

#[test_case]
fn example() {
    assert!(true);
}
