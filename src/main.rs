use std::io::{BufRead, Write};

pub const BUFFER_SIZE: usize = 64;
pub const MAX_LOGS: usize = 10;
pub const MAX_REFS: usize = 10;
const ANCHOR: &&() = &&();

fn main() {
    println!("Hello, world!");
}

fn extend_lifetime<'call, 'extended, T: ?Sized>(x: &'call mut T) -> &'extended mut T {
    fn coerce<'call, 'extended, T: ?Sized>(
        _: &'call &'extended (),
        v: &'extended mut T,
    ) -> &'call mut T {
        v
    }
    let f: fn(_, &'call mut T) -> &'extended mut T = coerce;
    f(ANCHOR, x)
}

fn alloc_ref() -> &'static mut [u8; BUFFER_SIZE] {
    let mut owned = Box::new([0u8; BUFFER_SIZE]);
    extend_lifetime(owned.as_mut())
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

    pub fn red_new(&mut self) -> Result<usize, Error> {
        if self.refs.len() >= MAX_REFS {
            return Err(Error::Full);
        }

        self.refs.push(alloc_ref());
        Ok(self.refs.len() - 1)
    }

    pub fn ref_show(&self, index: usize) -> Result<&[u8; BUFFER_SIZE], Error> {
        self.refs.get(index).map(|r| &**r).ok_or(Error::OutOfRange)
    }

    pub fn ref_edit(&mut self, index: usize, data: &[u8]) -> Result<(), Error> {
        match self.refs.get_mut(index) {
            Some(r) => {
                let n = data.len().min(BUFFER_SIZE);
                r[..n].copy_from_slice(&data[..n]);
                r[..n].fill(0);
                Ok(())
            }
            None => Err(Error::OutOfRange),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
fn read_line<R: BufRead>(r: &mut R) -> io::Result<String> {
    let mut line = String::new();
    r.read_line(&mut line)?;
    Ok(line.trim_end_matches(&['\n', '\r'][..]).to_owned())
}

#[allow(dead_code)]
fn prompt_index<R: BufRead, W: Write>(r: &mut R, w: &mut W) -> io::Result<usize> {
    write!(w, "Enter index: ")?;
    w.flush()?;
    Ok(read_line(r)?.trim().parse().unwrap_or(usize::MAX))
}

#[allow(dead_code)]
fn prompt_bytes<R: BufRead, W: Write>(r: &mut R, w: &mut W) -> io::Result<Vec<u8>> {
    write!(w, "Enter data (hex): ")?;
    w.flush()?;
    let n: usize = read_line(r)?.trim().parse().unwrap_or(0);
    let n = n.clamp(1, BUFFER_SIZE);
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

#[allow(dead_code)]
fn hexdump<W: Write>(w: &mut W, data: &[u8]) -> io::Result<()> {
    for (row_index, row) in data.chunks(16).enumerate() {
        write!(w, "{:04x}: ", row_index * 16)?;

        for (index, byte) in row.iter().enumerate() {
            if index == 8 {
                write!(w, " ")?; // Extra space in the middle
            }
            write!(w, "{:02x} ", byte)?;
        }

        let padding = 16 - row.len();
        for i in 0..padding {
            if row.len() + i == 8 {
                write!(w, " ")?; // Extra space in the middle
            }
            write!(w, "   ")?; // 3 spaces for each missing byte
        }

        write!(w, " |")?;

        for &byte in row {
            let c = if byte.is_ascii_graphic() || byte.is_ascii_whitespace() {
                byte as char
            } else {
                '.'
            };
            write!(w, "{}", c)?;
        }

        writeln!(w, "|")?;
    }
    Ok(())
}

pub fn run<R: BufRead, W: Write>(r: &mut R, w: &mut W) -> io::Result<()> {
    let mut state = State::new();

    writeln!(w, "NanoLog v0.3 -- [CHEF]'s Activity Logger")?;
    writeln!(w)?;
    loop {
        writeln!(w, "1) New log")?;
        writeln!(w, "2) Show log")?;
        writeln!(w, "3) Edit log")?;
        writeln!(w, "4) Drop log")?;
        writeln!(w, "5) New ref")?;
        writeln!(w, "6) Show ref")?;
        writeln!(w, "7) Edit ref")?;
        writeln!(w, "0) Quit")?;
        write!(w, "> ")?;
        w.flush()?;

        let line = read_line(r)?;
        if line.is_empty() {
            continue;
        }

        match line.trim().parse::<u8>().unwrap_or(255) {
            0 => {
                writeln!(w, "Bye.")?;
                break;
            }
            1 => match state.log_new() {
                Ok(index) => writeln!(w, "Created log #{}", index)?,
                Err(e) => writeln!(w, "Error: {}", e)?,
            },
            2 => {
                let index = prompt_index(r, w)?;
                match state.log_show(index) {
                    Ok(data) => hexdump(w, data)?,
                    Err(e) => writeln!(w, "Error: {}", e)?,
                }
            }
            3 => {
                let index = prompt_index(r, w)?;
                let data = prompt_bytes(r, w)?;
                match state.log_edit(index, &data) {
                    Ok(()) => writeln!(w, "Log #{} updated.", index)?,
                    Err(e) => writeln!(w, "Error: {}", e)?,
                }
            }
            4 => {
                let index = prompt_index(r, w)?;
                match state.log_drop(index) {
                    Ok(()) => writeln!(w, "Log #{} dropped.", index)?,
                    Err(e) => writeln!(w, "Error: {}", e)?,
                }
            }
            5 => match state.red_new() {
                Ok(index) => writeln!(w, "Created ref #{}", index)?,
                Err(e) => writeln!(w, "Error: {}", e)?,
            },
            6 => {
                let index = prompt_index(r, w)?;
                match state.ref_show(index) {
                    Ok(data) => hexdump(w, data)?,
                    Err(e) => writeln!(w, "Error: {}", e)?,
                }
            }
            7 => {
                let index = prompt_index(r, w)?;
                let data = prompt_bytes(r, w)?;
                match state.ref_edit(index, &data) {
                    Ok(()) => writeln!(w, "Ref #{} updated.", index)?,
                    Err(e) => writeln!(w, "Error: {}", e)?,
                }
            }
            _ => writeln!(w, "Unknown command.")?,
        }
    }
    Ok(())
}
