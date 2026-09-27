//! The colour contract of `src/ui.rs`: styling is applied unconditionally with `owo_colors` and it
//! is the output stream — `anstream` — that decides whether the escapes survive.
//!
//! That decision cannot be observed from inside the test process, whose own stdout is whatever the
//! harness hands it, so each case runs the real binary with stdout on a pipe. `CLICOLOR_FORCE` is
//! what makes the assertions falsifiable: it proves the escapes exist and are being deliberately
//! stripped, rather than never having been produced.

use std::path::Path;
use std::process::{Command, Output};

const ESCAPE: u8 = 0x1b;

fn run(source: &Path, env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_uncomment"));
    command.arg("--dry-run").arg(source);
    // A colour choice inherited from the ambient environment would decide the result instead of the
    // case under test.
    for name in ["NO_COLOR", "CLICOLOR", "CLICOLOR_FORCE", "TERM"] {
        command.env_remove(name);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("runs the uncomment binary")
}

fn source_file(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("colored.py");
    std::fs::write(&path, "value = 1  # removed\n").expect("writes the fixture");
    path
}

#[test]
fn colors_are_stripped_from_a_piped_stdout_and_restored_when_forced() {
    let temp = tempfile::TempDir::new().expect("creates a temp dir");
    let source = source_file(temp.path());

    let piped = run(&source, &[]);
    assert!(
        piped.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&piped.stderr)
    );
    assert!(
        !piped.stdout.contains(&ESCAPE),
        "a non-terminal stdout must carry no escapes: {:?}",
        String::from_utf8_lossy(&piped.stdout)
    );
    assert!(
        String::from_utf8_lossy(&piped.stdout).contains("Summary:"),
        "the summary itself must survive the stripping"
    );

    let forced = run(&source, &[("CLICOLOR_FORCE", "1")]);
    assert!(
        forced.stdout.contains(&ESCAPE),
        "CLICOLOR_FORCE must keep the escapes, otherwise the case above proves nothing: {:?}",
        String::from_utf8_lossy(&forced.stdout)
    );
}

#[test]
fn no_color_wins_over_a_forced_color_choice() {
    let temp = tempfile::TempDir::new().expect("creates a temp dir");
    let source = source_file(temp.path());

    let output = run(&source, &[("CLICOLOR_FORCE", "1"), ("NO_COLOR", "1")]);
    assert!(
        !output.stdout.contains(&ESCAPE),
        "NO_COLOR must suppress colour even where it was forced on: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !output.stderr.contains(&ESCAPE),
        "NO_COLOR applies to stderr as well: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}
