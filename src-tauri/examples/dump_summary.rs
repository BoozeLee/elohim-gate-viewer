//! Print the summary a fixture would produce, as the JSON the frontend consumes.
//!
//! Used by `tools/render_shots.py` to feed the render harness. Going through the
//! same `summarize` the app calls is the point: a harness fed hand-written JSON
//! would prove only that the template renders the shape somebody guessed.

use elohim_gate_viewer_lib::testing::{run_from_json, summarize};

fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| {
        eprintln!("usage: dump_summary <fixture.json> <exit_code>");
        std::process::exit(2);
    });
    let exit_code: i32 = args
        .next()
        .unwrap_or_else(|| {
            eprintln!("usage: dump_summary <fixture.json> <exit_code>");
            std::process::exit(2);
        })
        .parse()
        .expect("exit code must be an integer");

    let run = run_from_json(&name, exit_code);
    let summary = summarize(&run).unwrap_or_else(|error| {
        eprintln!("{name}: {error}");
        std::process::exit(1);
    });
    println!(
        "{}",
        serde_json::to_string(&summary).expect("summary serializes")
    );
}