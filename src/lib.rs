use std::io::{self, BufRead, Write};

pub const BUFFER_SIZE: usize = 144;
pub const MAX_LOGS: usize = 25;
pub const MAX_REFS: usize = 10;
const ANCHOR: &&() = &&();

fn cache_ref<'call, 'extended, T: ?Sized>(x: &'call mut T) -> &'extended mut T {
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

    #[cfg(feature = "heap-debug")]
    eprintln!("alloc_ref = {:p}", owned.as_mut_ptr());

    cache_ref(owned.as_mut())
}

#[repr(C)]
pub struct AdminRecord {
    is_admin: u64,
    username: [u8; BUFFER_SIZE - 8],
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

pub struct State {
    admins: Vec<Option<Box<AdminRecord>>>,
    logs: Vec<Option<Box<[u8; BUFFER_SIZE]>>>,
    refs: Vec<&'static mut [u8; BUFFER_SIZE]>,
}

impl State {
    pub fn new() -> Self {
        Self {
            admins: Vec::with_capacity(MAX_LOGS),
            logs: Vec::with_capacity(MAX_LOGS),
            refs: Vec::with_capacity(MAX_REFS),
        }
    }

    pub fn admin_new(&mut self) -> Result<usize, Error> {
        if self.admins.len() >= MAX_LOGS {
            return Err(Error::Full);
        }

        self.admins.push(Some(Box::new(AdminRecord {
            is_admin: 0,
            username: [0u8; BUFFER_SIZE - 8],
        })));

        Ok(self.admins.len() - 1)
    }

    pub fn admin_show(&self, index: usize) -> Result<&AdminRecord, Error> {
        match self.admins.get(index) {
            Some(Some(admin)) => Ok(admin),
            Some(None) => Err(Error::Deleted),
            None => Err(Error::OutOfRange),
        }
    }

    pub fn admin_flag<W: Write>(&mut self, index: usize, w: &mut W) -> Result<(), Error> {
        match self.admins.get_mut(index) {
            Some(Some(admin)) => {
                if admin.is_admin == 1 {
                    let flag = std::env::var("FLAG").expect("FLAG not set");
                    writeln!(w, "Congratulations! {}", flag).map_err(|_| Error::Deleted)?;
                    Ok(())
                } else {
                    Err(Error::Deleted)
                }
            }
            Some(None) => Err(Error::Deleted),
            None => Err(Error::OutOfRange),
        }
    }

    pub fn log_new(&mut self) -> Result<usize, Error> {
        if self.logs.len() >= MAX_LOGS {
            return Err(Error::Full);
        }

        #[allow(unused_mut)]
        let mut b = Box::new([0u8; BUFFER_SIZE]);

        #[cfg(feature = "heap-debug")]
        eprintln!("log_new  = {:p}", b.as_mut_ptr());

        self.logs.push(Some(b));

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
                b[n..].fill(0);
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

    pub fn ref_new(&mut self) -> Result<usize, Error> {
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
                r[n..].fill(0);
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

fn read_line<R: BufRead>(r: &mut R) -> io::Result<String> {
    let mut line = String::new();
    r.read_line(&mut line)?;
    Ok(line.trim_end_matches(&['\n', '\r'][..]).to_owned())
}

fn prompt_index<R: BufRead, W: Write>(r: &mut R, w: &mut W) -> io::Result<usize> {
    write!(w, "Enter index: ")?;
    w.flush()?;
    Ok(read_line(r)?.trim().parse().unwrap_or(usize::MAX))
}

fn prompt_bytes<R: BufRead, W: Write>(r: &mut R, w: &mut W) -> io::Result<Vec<u8>> {
    write!(w, "Enter data (hex): ")?;
    w.flush()?;
    let n: usize = read_line(r)?.trim().parse().unwrap_or(0);
    let n = n.clamp(1, BUFFER_SIZE);
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    let mut discard = String::new();
    r.read_line(&mut discard)?;
    Ok(buf)
}

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
            let c = if byte.is_ascii_graphic() || byte == b' ' {
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

    writeln!(w, "NanoLog v0.3 -- [C.GPT]'s Activity Logger")?;
    writeln!(w)?;
    loop {
        writeln!(w, "1) New log")?;
        writeln!(w, "2) Show log")?;
        writeln!(w, "3) Edit log")?;
        writeln!(w, "4) Drop log")?;
        writeln!(w, "5) New ref")?;
        writeln!(w, "6) Show ref")?;
        writeln!(w, "7) Edit ref")?;

        if !state.refs.is_empty() {
            writeln!(w, "8) New admin")?;
        }

        if !state.admins.is_empty() {
            writeln!(w, "9) Show admin")?;
        }

        if state
            .admins
            .iter()
            .any(|admin| matches!(admin, Some(a) if a.is_admin == 1))
        {
            writeln!(w, "10) Get flag")?;
        }

        writeln!(w, "0) Quit")?;
        write!(w, "> ")?;
        w.flush()?;

        let line = read_line(r)?;
        if line.is_empty() {
            break;
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
            5 => match state.ref_new() {
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
            8 => match state.admin_new() {
                Ok(index) => writeln!(w, "Created admin #{}", index)?,
                Err(e) => writeln!(w, "Error: {}", e)?,
            },
            9 => {
                let index = prompt_index(r, w)?;
                match state.admin_show(index) {
                    Ok(admin) => {
                        writeln!(w, "Is admin : {}", admin.is_admin)?;
                    }
                    Err(e) => writeln!(w, "Error: {}", e)?,
                }
            }
            10 => {
                let index = prompt_index(r, w)?;
                match state.admin_flag(index, w) {
                    Ok(()) => {}
                    Err(e) => writeln!(w, "Error: {}", e)?,
                }
            }
            _ => writeln!(w, "Unknown command.")?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[allow(dead_code)]
    fn session(input: &[u8]) -> String {
        let mut r = Cursor::new(input.to_vec());
        let mut w = Vec::new();
        run(&mut r, &mut w).unwrap();
        String::from_utf8_lossy(&w).into_owned()
    }

    #[allow(dead_code)]
    fn edit_cmd(cmd: u8, index: usize, data: &[u8]) -> Vec<u8> {
        let mut v = format!("{cmd}\n{index}\n{}\n", data.len()).into_bytes();
        v.extend_from_slice(data);
        v.push(b'\n');
        v
    }
    #[test]
    fn state_log_new_sequential_indices() {
        let mut state = State::new();
        assert_eq!(state.log_new(), Ok(0));
        assert_eq!(state.log_new(), Ok(1));
    }

    #[test]
    fn state_log_show_default_zeroed() {
        let mut state = State::new();
        state.log_new().unwrap();
        assert_eq!(state.log_show(0).unwrap(), &[0u8; BUFFER_SIZE]);
    }

    #[test]
    fn state_log_edit_writes_data() {
        let mut state = State::new();
        state.log_new().unwrap();
        state.log_edit(0, b"Hello").unwrap();
        assert_eq!(&state.log_show(0).unwrap()[..5], b"Hello");
    }

    #[test]
    fn state_log_edit_short_data_zeroes_tail() {
        let mut state = State::new();
        state.log_new().unwrap();
        state.log_edit(0, &[0xffu8; BUFFER_SIZE]).unwrap();
        state.log_edit(0, b"Hi").unwrap();
        let buffer = state.log_show(0).unwrap();
        assert_eq!(&buffer[..2], b"Hi");
        assert!(buffer[2..].iter().all(|&b| b == 0));
    }

    #[test]
    fn state_log_edit_clamps_to_buffer_size() {
        let mut state = State::new();
        state.log_new().unwrap();
        state.log_edit(0, &[0xAAu8; BUFFER_SIZE + 32]).unwrap();
        assert_eq!(state.log_show(0).unwrap(), &[0xAAu8; BUFFER_SIZE]);
    }

    #[test]
    fn state_log_drop_prevents_read() {
        let mut state = State::new();
        state.log_new().unwrap();
        state.log_drop(0).unwrap();
        assert_eq!(state.log_show(0), Err(Error::Deleted));
    }

    #[test]
    fn state_log_drop_prevents_write() {
        let mut state = State::new();
        state.log_new().unwrap();
        state.log_drop(0).unwrap();
        assert_eq!(state.log_edit(0, b"X"), Err(Error::Deleted));
    }

    #[test]
    fn state_log_double_drop_errors() {
        let mut state = State::new();
        state.log_new().unwrap();
        state.log_drop(0).unwrap();
        assert_eq!(state.log_drop(0), Err(Error::Deleted));
    }

    #[test]
    fn state_log_full_rejects_new() {
        let mut state = State::new();
        for _ in 0..MAX_LOGS {
            state.log_new().unwrap();
        }
        assert_eq!(state.log_new(), Err(Error::Full));
    }

    #[test]
    fn state_oob_read_errors() {
        let state = State::new();
        assert_eq!(state.log_show(0), Err(Error::OutOfRange));
        assert_eq!(state.ref_show(0), Err(Error::OutOfRange));
    }

    #[test]
    fn state_oob_edit_errors() {
        let mut state = State::new();
        assert_eq!(state.log_edit(0, b"X"), Err(Error::OutOfRange));
        assert_eq!(state.ref_edit(0, b"X"), Err(Error::OutOfRange));
    }

    #[test]
    fn state_ref_new_sequential_indices() {
        let mut state = State::new();
        assert_eq!(state.ref_new(), Ok(0));
        assert_eq!(state.ref_new(), Ok(1));
    }

    #[test]
    fn state_ref_edit_writes_data() {
        let mut state = State::new();
        state.ref_new().unwrap();
        state.log_new().unwrap();
        state.ref_edit(0, b"Hello").unwrap();
        assert_eq!(&state.ref_show(0).unwrap()[..5], b"Hello");
    }

    #[test]
    fn state_ref_full_rejects_new() {
        let mut state = State::new();
        for _ in 0..MAX_REFS {
            state.ref_new().unwrap();
        }
        assert_eq!(state.ref_new(), Err(Error::Full));
    }

    #[test]
    fn uaf_ref_aliases_subsequent_log() {
        let shared = alloc_ref();
        let mut owned = Box::new([0u8; BUFFER_SIZE]);
        owned[0] = 0xca;
        owned[1] = 0xfe;
        owned[2] = 0xba;
        owned[3] = 0xbe;
        assert_eq!(shared[..4], [0xca, 0xfe, 0xba, 0xbe]);
        drop(owned);
    }

    #[test]
    fn uaf_ref_observes_log_write() {
        let mut state = State::new();
        state.ref_new().unwrap();
        state.log_new().unwrap();
        state.log_edit(0, &[0xdeu8; BUFFER_SIZE]).unwrap();
        assert_eq!(state.ref_show(0).unwrap(), &[0xdeu8; BUFFER_SIZE]);
    }

    #[test]
    fn uaf_ref_write_visible_in_log() {
        let mut state = State::new();
        state.ref_new().unwrap();
        state.log_new().unwrap();
        state.log_edit(0, b"original").unwrap();
        state.ref_edit(0, b"patched").unwrap();
        assert_eq!(&state.log_show(0).unwrap()[..7], b"patched");
    }

    #[test]
    fn protocol_new_log_prints_index() {
        let out = session(b"1\n0\n");
        assert!(out.contains("Created log #0"));
    }

    #[test]
    fn protocol_show_new_log_is_zeroed() {
        let out = session(b"1\n2\n0\n0\n");
        assert!(out.contains("00 00 00 00"));
    }

    #[test]
    fn protocol_edit_and_show_roundtrip() {
        let mut input = b"1\n".to_vec();
        input.extend(edit_cmd(3, 0, b"AAAA"));
        input.extend(b"2\n0\n0\n");
        let out = session(&input);
        assert!(out.contains("41 41 41 41"));
    }

    #[test]
    fn protocol_drop_log_prints_dropped() {
        let out = session(b"1\n4\n0\n0\n");
        assert!(out.contains("dropped"));
    }

    #[test]
    fn protocol_new_ref_prints_index() {
        let out = session(b"5\n0\n");
        assert!(out.contains("Created ref #0"));
    }

    #[test]
    fn protocol_unknown_command_continues() {
        let out = session(b"99\n0\n");
        assert!(out.contains("Unknown command"));
    }

    #[test]
    fn protocol_quit() {
        let out = session(b"0\n");
        assert!(out.contains("Bye."));
    }

    #[test]
    fn protocol_oob_index_errors() {
        let out = session(b"2\n99\n0\n");
        assert!(out.contains("Error:"));
    }
}
