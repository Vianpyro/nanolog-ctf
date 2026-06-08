use std::io::{self, BufWriter};

const WELCOME_MESSAGE: &str = "\
 _______                       .____                         _______       ________
 ╲      ╲ _____    ____   ____ │    │    ____   ____   ___  _╲   _  ╲      ╲_____  ╲
 ╱   │   ╲╲__  ╲  ╱    ╲ ╱  _ ╲│    │   ╱  _ ╲ ╱ ___╲  ╲  ╲╱ ╱  ╱_╲  ╲       _(__  <
╱    │    ╲╱ __ ╲│   │  (  <_> )    │__(  <_> ) ╱_╱  >  ╲   ╱╲  ╲_╱   ╲     ╱       ╲
╲____│__  (____  ╱___│  ╱╲____╱│_______ ╲____╱╲___  ╱    ╲_╱  ╲_____  ╱ ╱╲ ╱______  ╱
        ╲╱     ╲╱     ╲╱               ╲╱    ╱_____╱                ╲╱  ╲╱        ╲╱

[SYS] Database restored successfully.
[SYS] 0 logs recovered.
[SYS] 0 administrators recovered.
[SYS] Warning: reference cache contains stale entries.
";

fn main() -> io::Result<()> {
    std::env::var("FLAG1").expect("FLAG1 environment variable not set -- refusing to start");
    std::fs::read_to_string("/flag").expect("/flag file not found or empty -- refusing to start");

    println!("{WELCOME_MESSAGE}");

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    nanolog::run(&mut stdin.lock(), &mut writer)
}
