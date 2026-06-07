use std::io::{self, BufWriter};

fn main() -> io::Result<()> {
    std::env::var("FLAG").expect("FLAG environment variable not set -- refusing to start");

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    nanolog::run(&mut stdin.lock(), &mut writer)
}
