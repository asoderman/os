use core::fmt::Write;

pub struct StdOut<W: Write> {
    out: W
}

impl<W:Write> StdOut<W> {
    pub fn new(w: W) -> Self {
        StdOut {
            out: w
        }
    }
}
