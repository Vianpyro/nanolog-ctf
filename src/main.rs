use std::io::{BufRead, Write};

pub const BUFFER_SIZE: usize = 64;
pub const MAX_LOGS: usize = 10;
pub const MAX_REFS: usize = 10;

fn main() {
    println!("Hello, world!");
}

#[derive(Debug, PartialEq)]
pub enum Error {
    Full,
    OutOfRange,
    Deleted,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Full => write!(f, "Error: Storage is full"),
            Error::OutOfRange => write!(f, "Error: Index out of range"),
            Error::Deleted => write!(f, "Error: Entry was deleted"),
        }
    }
}

#[allow(dead_code)]
pub struct State {
    logs: Vec<Option<Box<[u8; BUFFER_SIZE]>>>,
    refs: Vec<&'static mut [u8; BUFFER_SIZE]>,
}

impl State {
    pub fn new() -> Self {
        State {
            logs: Vec::with_capacity(MAX_LOGS),
            refs: Vec::with_capacity(MAX_REFS),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

pub fn run<R: BufRead, W: Write>(_r: &mut R, _w: &mut W) -> Result<(), Error> {
    todo!()
}
