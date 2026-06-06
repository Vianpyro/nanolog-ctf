use std::io;

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    nanolog::run(&mut stdin.lock(), &mut stdout.lock())
}
