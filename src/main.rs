use std::io::{self, BufWriter};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    nanolog::run(&mut stdin.lock(), &mut writer)
}
