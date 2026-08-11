use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::process;

fn main() {
    let Some(output) = env::var_os("REWIRE_TEST_OUTPUT") else {
        process::exit(90);
    };
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let executable = env::current_exe().expect("resolve fixture executable");
    let mut writer = BufWriter::new(File::create(output).expect("create fixture output"));

    writeln!(writer, "executable={}", executable.display()).expect("write executable path");
    for argument in &arguments {
        writeln!(writer, "argument={argument}").expect("write fixture argument");
    }
    writer.flush().expect("flush fixture output");

    if arguments
        .first()
        .is_some_and(|value| value == "--fixture-exit")
    {
        let status = arguments
            .get(1)
            .and_then(|value| value.parse().ok())
            .unwrap_or(1);
        process::exit(status);
    }
}
