//! Render the real payloads in `fixtures/` and assert what a user would read.
//!
//! The unit tests in `gate.rs` use hand-built payloads to cover shapes a real
//! run has not produced yet. These use two captured real runs, because the thing
//! that breaks a viewer in production is not an exotic shape -- it is a key the
//! real gate emits that the viewer never saw while being written.
//!
//! See `fixtures/README.md` for what each fixture was made to be bad.

use elohim_gate_viewer_lib::testing::{run_from_json, summarize_payload};

fn clean() -> elohim_gate_viewer_lib::testing::GateSummary {
    summarize_payload(run_from_json("payload_clean.json", 0))
        .expect("the captured PASS payload must render")
}

fn fail() -> elohim_gate_viewer_lib::testing::GateSummary {
    summarize_payload(run_from_json("payload_fail.json", 1))
        .expect("a FAIL payload is still a payload and must render")
}

#[test]
fn the_healthy_run_renders_green_with_counts_that_add_up() {
    let summary = clean();
    assert_eq!(summary.schema, "elohim.gate/1");
    assert_eq!(summary.verdict, "PASS");
    assert_eq!(summary.exit_code, 0);
    assert_eq!(summary.rows.len(), 6, "six skills were gated");
    assert_eq!(summary.skills, 6);
    assert_eq!(summary.passed, 6);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.unlocated, 0);
    assert_eq!((summary.facts_verified, summary.facts), (81, 81));
    assert_eq!((summary.traps_holding, summary.traps), (38, 38));
    assert_eq!(summary.facts_drifted, 0);
    assert!(summary.rows.iter().all(|row| row.verdict == "PASS"));
    assert!(summary.rows.iter().all(|row| row.pin_status == "PASS"));
    assert!(
        summary.rows.iter().all(|row| row.problem.is_empty()),
        "a green run must have no per-row problem text to explain"
    );
}

#[test]
fn every_skill_pin_is_shown_short_and_never_in_full() {
    // The payload carries full 64-hex pins. A viewer that renders them whole
    // puts a wall of hex in front of the user and invites eyeball-comparing
    // values the gate already compared, which is the thing this project stopped
    // doing.
    let summary = clean();
    for row in &summary.rows {
        assert_eq!(row.pin_sha.len(), 12, "{} pin not shortened", row.skill);
        assert!(row.pin_sha.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

#[test]
fn the_failing_run_reports_both_failures_with_their_distinct_causes() {
    let summary = fail();
    assert_eq!(summary.verdict, "FAIL");
    assert_eq!(summary.exit_code, 1, "the gate exits 1 on a red verdict");
    assert_eq!(summary.rows.len(), 6, "a failing skill must not remove other rows");
    assert_eq!(summary.failed, 2);
    assert_eq!(summary.passed, 4);
    assert_eq!((summary.facts_verified, summary.facts), (80, 81));
    assert_eq!(summary.facts_drifted, 1);
    assert_eq!((summary.traps_holding, summary.traps), (37, 38));

    let elohim = summary
        .rows
        .iter()
        .find(|row| row.skill == "elohim")
        .expect("elohim is a gated skill");
    assert_eq!(elohim.verdict, "FAIL");
    assert_eq!(elohim.pin_status, "DRIFT", "failure 1 is caught by the pin");
    assert!(
        elohim.pin_detail.contains("expected") && elohim.pin_detail.contains("found"),
        "a DRIFT row must carry the gate's own expected/actual wording, got {:?}",
        elohim.pin_detail
    );
    assert_eq!(
        (elohim.facts_verified, elohim.facts_total),
        (25, 25),
        "elohim's own facts were untouched; only the pin moved"
    );

    let reproducibility = summary
        .rows
        .iter()
        .find(|row| row.skill == "reproducibility")
        .expect("reproducibility is a gated skill");
    assert_eq!(reproducibility.verdict, "FAIL");
    assert_eq!(
        reproducibility.pin_status, "PASS",
        "failure 2 is NOT a pin failure -- rendering it as one would be wrong"
    );
    assert_eq!(
        (reproducibility.facts_verified, reproducibility.facts_total),
        (5, 6),
        "its own fact went drifted even though its pin is intact"
    );
}

#[test]
fn a_failed_skill_is_never_rendered_as_a_healthy_row() {
    // The failure mode this whole project exists to prevent: a red skill shown
    // as a blank cell, or as 0/0 verified, which reads as "nothing to worry
    // about". A FAIL row must show at least one thing actually wrong with it --
    // a drifted pin, a drifted fact, or a trap that did not hold.
    let summary = fail();
    for row in &summary.rows {
        if row.verdict == "PASS" {
            continue;
        }
        let something_is_wrong = row.pin_status != "PASS"
            || row.facts_verified < row.facts_total
            || row.traps_holding < row.traps_total;
        assert!(
            something_is_wrong,
            "{} is {} but the row shows no drifted pin, no drifted fact, and \
             no failing trap -- the user is shown red with nothing to read",
            row.skill,
            row.verdict
        );
        assert!(
            !row.pin_detail.is_empty() || row.problem.is_empty(),
            "{} is FAIL and has a problem, so it must also explain it",
            row.skill
        );
    }
}

#[test]
fn the_run_says_which_command_produced_it_and_which_schema_it_read() {
    // Provenance on screen. A verdict with no schema and no command is a claim
    // from nowhere, which is precisely the class of claim the gate forbids.
    let summary = clean();
    assert_eq!(summary.schema, "elohim.gate/1");
    assert!(
        summary.command.contains("--all") && summary.command.contains("--json"),
        "got {:?}",
        summary.command
    );
}

#[test]
fn no_host_path_reaches_the_rendered_summary() {
    // `contract.md` says the payload's paths are not portable and consumers must
    // not key on them. The cheapest way to hold to that is to prove none of them
    // survive into what the user sees.
    let summary = clean();
    let rendered = format!("{summary:?}");
    assert!(
        !rendered.contains("/srv/elohim-checkout") && !rendered.contains("/home/"),
        "a host path leaked into the summary: {rendered}"
    );
}