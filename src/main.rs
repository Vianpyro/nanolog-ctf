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
        Self {
            logs: Vec::with_capacity(MAX_LOGS),
            refs: Vec::with_capacity(MAX_REFS),
        }
    }

    pub fn log_new(&mut self) -> Result<usize, Error> {
        if self.logs.len() >= MAX_LOGS {
            return Err(Error::Full);
        }

        self.logs.push(Some(Box::new([0u8; BUFFER_SIZE])));

        Ok(self.logs.len() - 1)
    }

    pub fn log_show(&self, index: usize) -> Result<&[u8; BUFFER_SIZE], Error> {
        match self.logs.get(index) {
            Some(Some(b)) => Ok(b),
            Some(None) => Err(Error::Deleted),
            None => Err(Error::OutOfRange),
        }
    }

    pub fn log_edit(&mut self, index: usize, data: &[u8]) -> Result<(), Error> {
        match self.logs.get_mut(index) {
            Some(Some(b)) => {
                let n = data.len().min(BUFFER_SIZE);
                b[..n].copy_from_slice(&data[..n]);
                b[..n].fill(0);
                Ok(())
            }
            Some(None) => Err(Error::Deleted),
            None => Err(Error::OutOfRange),
        }
    }

    pub fn log_drop(&mut self, index: usize) -> Result<(), Error> {
        match self.logs.get_mut(index) {
            Some(slot @ Some(_)) => {
                *slot = None;
                Ok(())
            }
            Some(None) => Err(Error::Deleted),
            None => Err(Error::OutOfRange),
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
