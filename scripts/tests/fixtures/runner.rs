use std::env;
use std::fs::File;
use std::io::{BufWriter, IsTerminal, Read, Write};
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
    if env::var_os("REWIRE_TEST_RECORD_TERMINAL").is_some() {
        writeln!(writer, "stdin-is-terminal={}", std::io::stdin().is_terminal())
            .expect("write fixture terminal state");
    }
    writer.flush().expect("flush fixture output");

    if env::var_os("REWIRE_TEST_REQUIRE_TERMINAL").is_some()
        && !std::io::stdin().is_terminal()
    {
        process::exit(91);
    }
    if arguments
        .first()
        .is_some_and(|value| value == "--fixture-read-stdin")
    {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .expect("read fixture stdin");
        let mut writer = BufWriter::new(
            File::options()
                .append(true)
                .open(env::var_os("REWIRE_TEST_OUTPUT").unwrap())
                .expect("reopen fixture output"),
        );
        writeln!(writer, "stdin={}", input.trim_end()).expect("write fixture stdin");
        writer.flush().expect("flush fixture stdin");
    }

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
