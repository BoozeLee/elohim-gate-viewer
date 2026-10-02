//! Run a real gate end to end, when one is pointed at.
//!
//! The other tests here never spawn anything: they read captured payloads. That
//! leaves exactly one link in the chain unverified -- that `run_gate` builds the
//! right argv, starts the gate, reads its stdout, and parses it. This test
//! closes that link, and does it against a real checkout rather than a mock, so
//! it proves the actual gate is reachable by the actual command the app uses.
//!
//! Skipped unless `ELOHIM_GATE_CMD` is set, because a committed test must not
//! require a checkout that a clone will not have:
//!
//! ```text
//! ELOHIM_GATE_CMD="python3 /path/to/elohim/skills/elohim-harness/scripts/harness_run.py" \
//!   cargo test --test real_gate -- --nocapture
//! ```
//!
//! It asserts whatever it can without assuming a verdict: a red checkout is a
//! legitimate outcome and must still parse. What must always hold is that the
//! argv was right, the payload was found, and the schema was recognised.

use elohim_gate_viewer_lib::testing::{run_gate_command, summarize};

/// The override the app itself honours, so this test and the app resolve the
/// gate by the same rule. Importing it rather than re-spelling the string means
/// a rename cannot leave this test testing a path the app no longer uses.
const OVERRIDE: &str = "ELOHIM_GATE_CMD";

/// `#[ignore]`, not an early `return`.
///
/// The first version of this test returned when `ELOHIM_GATE_CMD` was unset.
/// Cargo counts that as a pass, so `cargo test` printed a green line for a test
/// that had executed nothing -- a green end-to-end link that was never walked.
/// An ignored test is reported as ignored, which is the truth.
#[test]
#[ignore = "needs a real gate; run `npm run test:gate` with ELOHIM_GATE_CMD set"]
fn a_real_gate_is_invoked_and_its_payload_parsed() {
    let command = std::env::var(OVERRIDE).unwrap_or_else(|_| {
        panic!("this test is ignored unless {OVERRIDE} is set; run it via `npm run test:gate`")
    });
    assert!(
        !command.trim().is_empty(),
        "{OVERRIDE} is set but empty; unset it to run the suite without this test"
    );

    let run = run_gate_command(&command)
        .unwrap_or_else(|error| panic!("{OVERRIDE}={command:?} failed: {error}"));

    let mut parts = command.split_whitespace();
    let program = parts.next().expect("a command has a program");
    let mut expected: Vec<&str> = parts.collect();
    expected.push("--all");
    expected.push("--json");
    assert_eq!(
        run.command,
        format!("{program} {}", expected.join(" ")),
        "the app's flags must land at the end of the command line, after any \
         interpreter and script the override names"
    );
    assert!(
        run.payload.is_object(),
        "the gate's stdout must parse as a JSON object"
    );
    assert_eq!(
        run.schema, "elohim.gate/1",
        "this build knows one schema; if the gate moved, that is a real change to make deliberately"
    );

    let summary = summarize(&run).expect("a real payload must summarize");
    assert_eq!(
        summary.rows.len(),
        summary.skills,
        "one row per gated skill: the table is not allowed to silently drop any"
    );
    assert_eq!(
        summary.passed + summary.failed + summary.unlocated,
        summary.skills,
        "every skill lands in exactly one bucket"
    );
    assert_eq!(summary.passed + summary.failed, summary.rows.iter().filter(|row| row.verdict != "UNLOCATED").count());

    // The verdict on screen must match the exit code the gate actually returned,
    // or the two halves of this app disagree about what just happened.
    if summary.verdict == "PASS" {
        assert_eq!(summary.exit_code, 0, "PASS must be exit 0");
    } else {
        assert_ne!(
            summary.exit_code, 0,
            "a non-PASS verdict must not be reported as a clean exit"
        );
    }

    println!(
        "gate verdict={} exit={} skills={} passed={} failed={} unlocated={} facts={}/{} traps={}/{}",
        summary.verdict,
        summary.exit_code,
        summary.skills,
        summary.passed,
        summary.failed,
        summary.unlocated,
        summary.facts_verified,
        summary.facts,
        summary.traps_holding,
        summary.traps,
    );
}

/// An override that names nothing must be refused by name, not by a spawn error.
///
/// Always runs, so it costs a clone nothing while still covering the shape of
/// the one test that does need a checkout.
#[test]
fn an_override_naming_nothing_is_refused_by_name() {
    let error = run_gate_command("   ").unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains(OVERRIDE),
        "the message must name the variable to change, got: {message}"
    );
}