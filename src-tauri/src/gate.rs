//! Locating and running the elohim gate, and refusing anything it cannot trust.
//!
//! Kept free of Tauri types on purpose. Everything here is a pure function over a
//! command vector, so the paths a user actually hits -- gate missing, Python too
//! old, gate not printing JSON, gate printing a payload this build does not
//! understand -- are exercised by `cargo test` without a window ever opening. A
//! gate viewer that has only ever been seen green is untested, and this is the
//! half that decides whether the numbers on screen are real.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The lowest interpreter the gate itself declares support for. Read from
/// `elohim`'s `pyproject.toml` (`requires-python = ">=3.10"`) rather than chosen
/// here, so this build cannot drift into asking for something the gate refuses.
pub const MIN_PYTHON: (u32, u32) = (3, 10);

/// Payload shapes this build knows how to read.
///
/// A viewer that renders an unknown schema is worse than one that refuses: the
/// numbers on screen would come from a contract nobody pinned, which is the
/// defect the gate exists to catch. Bump this list when the gate's contract
/// moves, and only then.
pub const KNOWN_SCHEMAS: &[&str] = &["elohim.gate/1"];

/// Set by a developer or a test to run a specific gate. Without it the viewer
/// wants `elohim` on PATH, which is what an installed wheel provides.
pub const OVERRIDE_ENV: &str = "ELOHIM_GATE_CMD";

/// Why the gate could not be run, or why its output was not trusted.
///
/// Each variant names what was tried, because "could not find the gate" is only
/// actionable if it says where it looked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    /// No gate on PATH and no override set.
    NotFound { override_var: &'static str, path_entry: String },
    /// The override named a program that does not exist.
    OverrideMissing { command: String, override_var: &'static str },
    /// The gate could be started but not run.
    Spawn { command: String, detail: String },
    /// The gate ran and exited non-zero without printing a payload at all.
    NoPayload {
        command: String,
        exit_code: Option<i32>,
        stderr_tail: String,
    },
    /// The gate printed something that is not a JSON object.
    NotJson { command: String, detail: String, stdout_head: String },
    /// The payload has no `schema`, or one this build does not know.
    UnknownSchema { found: String, known: Vec<String> },
    /// A key the viewer needs is missing or the wrong shape.
    Malformed { detail: String },
    /// The interpreter is older than the gate supports.
    PythonTooOld {
        found: String,
        need: String,
        how_to_check: &'static str,
    },
}

impl std::fmt::Display for GateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateError::NotFound {
                override_var,
                path_entry,
            } => write!(
                f,
                "Could not find the elohim gate.\n\n\
                 The viewer runs the gate as a local process; it does not ship one.\n\n\
                   1. Install it:            pip install elohim\n\
                   2. Or point at one:       {override_var}=\"/path/to/elohim --all --json\"\n\n\
                 Looked for `elohim` on PATH (PATH={path_entry})."
            ),
            GateError::OverrideMissing {
                command,
                override_var,
            } => write!(
                f,
                "{override_var} is set to {command:?}, but that program does not exist.\n\
                 Unset it to fall back to `elohim` on PATH."
            ),
            GateError::Spawn { command, detail } => {
                write!(f, "Could not run {command:?}: {detail}")
            }
            GateError::NoPayload {
                command,
                exit_code,
                stderr_tail,
            } => {
                let code = match exit_code {
                    Some(code) => code.to_string(),
                    None => "a signal".to_string(),
                };
                write!(
                    f,
                    "{command} exited {code} without printing a payload, so there is \
                     nothing to show. This is a broken install, not a failing gate.\n\n\
                     Its last output was:\n\n{stderr_tail}"
                )
            }
            GateError::NotJson {
                command,
                detail,
                stdout_head,
            } => write!(
                f,
                "{command} printed something that is not a JSON payload: {detail}\n\n\
                 First bytes were:\n\n{stdout_head}"
            ),
            GateError::UnknownSchema { found, known } => write!(
                f,
                "The gate reported payload schema {found:?}, which this viewer does not \
                 understand.\n\nKnown: {}\n\nThis build will not render a payload whose shape \
                 nobody pinned. Update the viewer, or pin the gate to a version it reads.",
                known.join(", ")
            ),
            GateError::Malformed { detail } => {
                write!(f, "The gate's payload is not shaped as documented: {detail}")
            }
            GateError::PythonTooOld {
                found,
                need,
                how_to_check,
            } => write!(
                f,
                "The gate needs Python {need} or newer. Found {found}.\n\n\
                 Check with:\n  {how_to_check}\n\n\
                 The viewer refuses to guess which interpreter the gate would have used, \
                 because a gate that silently runs under a different interpreter is a \
                 gate whose numbers came from somewhere you did not choose."
            ),
        }
    }
}

impl std::error::Error for GateError {}

/// What a gate run produced. Kept alongside the parsed payload so a failing run
/// is still displayable: exit code, stderr, and whatever partial summary the
/// payload carried.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GateRun {
    /// The whole payload, passed to the frontend unchanged.
    pub payload: serde_json::Value,
    /// The gate's exit code. 0 is PASS; 1 is FAIL; 2 is UNLOCATED.
    pub exit_code: i32,
    /// The gate's stderr, for the failing case.
    pub stderr_tail: String,
    /// The command line actually run, so the user can see what was invoked.
    pub command: String,
    /// The schema, lifted out so the frontend need not reach into the payload.
    pub schema: String,
}

/// Which interpreter the gate would use, and whether it is new enough.
///
/// The gate is a Python program, so "is this machine able to run it" is a real
/// question with a real answer. Probed rather than assumed.
pub fn check_interpreter(python: &Path) -> Result<String, GateError> {
    let output = Command::new(python)
        .arg("-c")
        .arg("import sys; print('%d.%d.%d' % sys.version_info[:3])")
        .output()
        .map_err(|error| GateError::Spawn {
            command: python.display().to_string(),
            detail: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(GateError::Spawn {
            command: python.display().to_string(),
            detail: format!(
                "it exited {} for `-c`, so its version cannot be read",
                output.status
            ),
        });
    }
    let found = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !is_new_enough(&found) {
        return Err(GateError::PythonTooOld {
            found,
            need: format!("{}.{}", MIN_PYTHON.0, MIN_PYTHON.1),
            how_to_check: "python3 -V",
        });
    }
    Ok(found)
}

/// Whether a `major.minor[.patch]` version string clears MIN_PYTHON.
///
/// Returns false for anything it cannot read rather than assuming it is new.
/// An unparseable version is not evidence of a supported interpreter.
pub fn is_new_enough(version: &str) -> bool {
    let mut parts = version.trim().split('.');
    let major = parts.next().and_then(|part| part.trim().parse::<u32>().ok());
    let minor = parts.next().and_then(|part| part.trim().parse::<u32>().ok());
    match (major, minor) {
        (Some(major), Some(minor)) => (major, minor) >= MIN_PYTHON,
        _ => false,
    }
}

/// The gate command to run, from the override or from PATH.
///
/// Split on whitespace rather than taking a single path, because the natural
/// override is `--all --json` already attached and requiring the user to
/// re-quote it inside one variable is a papercut that produces a confusing
/// "program not found".
pub fn resolve_gate_command(env_override: Option<&str>) -> Result<String, GateError> {
    if let Some(raw) = env_override {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(GateError::OverrideMissing {
                command: trimmed.to_string(),
                override_var: OVERRIDE_ENV,
            });
        }
        let program = trimmed.split_whitespace().next().unwrap_or_default();
        return if program_is_runnable(program) {
            Ok(trimmed.to_string())
        } else {
            Err(GateError::OverrideMissing {
                command: trimmed.to_string(),
                override_var: OVERRIDE_ENV,
            })
        };
    }
    if program_is_runnable("elohim") {
        return Ok("elohim".to_string());
    }
    Err(GateError::NotFound {
        override_var: OVERRIDE_ENV,
        path_entry: std::env::var("PATH").unwrap_or_else(|_| "<unset>".to_string()),
    })
}

/// Split a command string into program and arguments.
pub fn split_command(command: &str) -> (String, Vec<String>) {
    let mut parts = command.split_whitespace().map(str::to_string);
    let program = parts.next().unwrap_or_default();
    (program, parts.collect())
}

/// Whether a program name can be resolved to something executable.
fn program_is_runnable(program: &str) -> bool {
    if program.is_empty() {
        return false;
    }
    // An explicit path is checked directly; a bare name is resolved against PATH
    // so that the same rule applies whether the user gave an absolute path or
    // just `elohim`.
    if program.contains(std::path::MAIN_SEPARATOR) {
        return Path::new(program).is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(program);
        candidate.is_file()
    })
}

/// Run the gate and parse its payload.
///
/// `command` is the full command line, so a test can point this at anything and
/// the real path through the code is the same one production takes.
pub fn run_gate(command: &str) -> Result<GateRun, GateError> {
    let (program, args) = split_command(command);
    // An all-whitespace command would otherwise reach the spawn and report
    // `No such file or directory` for a program named "", which sends the user
    // looking for a missing file instead of a blank setting. Found by the test
    // that asserts this error names the variable to change.
    if program.is_empty() {
        return Err(GateError::OverrideMissing {
            command: command.to_string(),
            override_var: OVERRIDE_ENV,
        });
    }
    let mut argv = args;
    // Appended at the END, not inserted after the program. The override is a
    // whole command line, and the most useful shape of it names an interpreter
    // and a script (`python3 /path/to/harness_run.py`). Putting the flags first
    // would give `python3 --all --json /path/to/harness_run.py`, where python3
    // claims `--all` as its own option and never runs the gate. Measured: the
    // failure was silent until run against a real gate, because no test that
    // reads a captured payload ever spawns anything.
    argv.push("--all".to_string());
    argv.push("--json".to_string());

    let output = Command::new(&program)
        .args(&argv)
        .output()
        .map_err(|error| GateError::Spawn {
            command: program.clone(),
            detail: error.to_string(),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stderr_tail: String = stderr.chars().rev().take(2000).collect::<Vec<_>>().into_iter().rev().collect();
    let exit_code = output.status.code().unwrap_or(-1);

    if stdout.trim().is_empty() {
        return Err(GateError::NoPayload {
            command: program,
            exit_code: output.status.code(),
            stderr_tail,
        });
    }

    let payload: serde_json::Value = serde_json::from_str(&stdout).map_err(|error| {
        GateError::NotJson {
            command: program.clone(),
            detail: error.to_string(),
            stdout_head: stdout.chars().take(300).collect(),
        }
    })?;
    if !payload.is_object() {
        return Err(GateError::NotJson {
            command: program,
            detail: format!("the top level is a {}, not an object", kind_of(&payload)),
            stdout_head: stdout.chars().take(300).collect(),
        });
    }

    let schema = schema_of(&payload)?;
    if !KNOWN_SCHEMAS.contains(&schema.as_str()) {
        return Err(GateError::UnknownSchema {
            found: schema,
            known: KNOWN_SCHEMAS.iter().map(|s| (*s).to_string()).collect(),
        });
    }

    Ok(GateRun {
        payload,
        exit_code,
        stderr_tail,
        command: format!("{program} {}", argv.join(" ")),
        schema,
    })
}

fn kind_of(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Lift and check `schema`.
///
/// A payload with no `schema` is treated as an unknown one rather than a
/// defaulted one: the whole point of the key is that a consumer can tell what
/// shape it is holding, and a missing key is exactly the case where it cannot.
pub fn schema_of(payload: &serde_json::Value) -> Result<String, GateError> {
    match payload.get("schema") {
        Some(serde_json::Value::String(text)) => Ok(text.clone()),
        Some(other) => Err(GateError::UnknownSchema {
            found: format!("a {} where a string belongs", kind_of(other)),
            known: KNOWN_SCHEMAS.iter().map(|s| (*s).to_string()).collect(),
        }),
        None => Err(GateError::UnknownSchema {
            found: "no schema key at all".to_string(),
            known: KNOWN_SCHEMAS.iter().map(|s| (*s).to_string()).collect(),
        }),
    }
}

/// The per-skill rows, read out of the payload by key.
///
/// This is the one place the viewer knows the payload's shape, and it reads only
/// keys the contract promises. A key that is missing becomes a rendered dash
/// rather than an exception, because a table that throws on one bad row shows
/// the user nothing at all.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct SkillRow {
    pub skill: String,
    pub verdict: String,
    pub pin_status: String,
    pub instrument_source: String,
    /// First 12 hex characters of the pin, or an empty string when unpinned.
    pub pin_sha: String,
    /// The gate's own explanation of the pin state. On a DRIFT row this is the
    /// most informative field in the payload -- it names the expected and actual
    /// bytes -- so a row that drops it leaves the user with a colour and no
    /// reason.
    pub pin_detail: String,
    pub facts_verified: usize,
    pub facts_total: usize,
    pub traps_holding: usize,
    pub traps_total: usize,
    pub timed_out: bool,
    /// Non-empty when the skill was not measured at all.
    pub problem: String,
}

/// The summary, read out of the payload by key.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct GateSummary {
    pub schema: String,
    pub verdict: String,
    pub exit_code: i32,
    pub skills: usize,
    pub passed: usize,
    pub failed: usize,
    pub unlocated: usize,
    pub facts_verified: usize,
    pub facts: usize,
    pub facts_drifted: usize,
    pub traps_holding: usize,
    pub traps: usize,
    pub hygiene_findings: usize,
    pub unbound_claims: usize,
    pub timed_out: usize,
    pub runtime_seconds: f64,
    pub rows: Vec<SkillRow>,
    pub stderr_tail: String,
    pub command: String,
}

/// Flatten a run into what the table renders.
pub fn summarize(run: &GateRun) -> Result<GateSummary, GateError> {
    let summary = run
        .payload
        .get("summary")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| GateError::Malformed {
            detail: "no `summary` object".to_string(),
        })?;
    let skills = run
        .payload
        .get("skills")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| GateError::Malformed {
            detail: "no `skills` array".to_string(),
        })?;

    let number = |map: &serde_json::Map<String, serde_json::Value>, key: &str| -> usize {
        map.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0) as usize
    };

    let rows = skills
        .iter()
        .map(|entry| {
            let empty = serde_json::Map::new();
            let map = entry.as_object().unwrap_or(&empty);
            let pin = map.get("instrument_pin").and_then(serde_json::Value::as_object);
            let pin_map = pin.unwrap_or(&empty);

            let facts = map.get("facts").and_then(serde_json::Value::as_array);
            let facts = facts.map(|items| items.as_slice()).unwrap_or(&[]);
            let facts_total = facts.len();
            let facts_verified = facts
                .iter()
                .filter(|fact| {
                    fact.get("status").and_then(serde_json::Value::as_str) == Some("verified")
                })
                .count();

            let traps = map.get("traps").and_then(serde_json::Value::as_array);
            let traps = traps.map(|items| items.as_slice()).unwrap_or(&[]);
            let traps_total = traps.len();
            let traps_holding = traps
                .iter()
                .filter(|trap| trap.get("pass").and_then(serde_json::Value::as_bool) == Some(true))
                .count();

            let problem = if number(map, "facts") == 0 && facts_total == 0 {
                map.get("instrument_error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };

            SkillRow {
                skill: map
                    .get("skill")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?")
                    .to_string(),
                verdict: map
                    .get("verdict")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?")
                    .to_string(),
                pin_status: pin_map
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?")
                    .to_string(),
                instrument_source: map
                    .get("instrument_source")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?")
                    .to_string(),
                pin_sha: pin_map
                    .get("expected_sha256")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .chars()
                    .take(12)
                    .collect(),
                pin_detail: pin_map
                    .get("detail")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                facts_verified,
                facts_total,
                traps_holding,
                traps_total,
                timed_out: map
                    .get("timed_out")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                problem,
            }
        })
        .collect();

    Ok(GateSummary {
        schema: run.schema.clone(),
        verdict: run
            .payload
            .get("verdict")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?")
            .to_string(),
        exit_code: run.exit_code,
        skills: number(summary, "skills"),
        passed: number(summary, "passed"),
        failed: number(summary, "failed"),
        unlocated: number(summary, "unlocated"),
        facts_verified: number(summary, "facts_verified"),
        facts: number(summary, "facts"),
        facts_drifted: number(summary, "facts_drifted"),
        traps_holding: number(summary, "traps_holding"),
        traps: number(summary, "traps"),
        hygiene_findings: number(summary, "hygiene_findings"),
        unbound_claims: number(summary, "unbound_claims"),
        timed_out: number(summary, "timed_out"),
        runtime_seconds: summary
            .get("runtime_seconds")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0),
        rows,
        stderr_tail: run.stderr_tail.clone(),
        command: run.command.clone(),
    })
}

/// The interpreter to probe: whatever the override or PATH offers.
///
/// Separate from `resolve_gate_command` because the two can disagree -- a
/// developer may point the gate at a checkout while their `python3` is a
/// different one -- and the viewer should say which interpreter it checked.
pub fn interpreter_to_probe() -> PathBuf {
    PathBuf::from("python3")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_is_read_componentwise() {
        assert!(is_new_enough("3.10.0"));
        assert!(is_new_enough("3.14.5"));
        assert!(is_new_enough("4.0.0"));
        assert!(!is_new_enough("3.9.18"));
        assert!(!is_new_enough("2.7.18"));
    }

    #[test]
    fn an_unreadable_version_is_not_treated_as_supported() {
        // The failure that matters: a probe that printed something unexpected
        // must not be read as permission to continue.
        assert!(!is_new_enough(""));
        assert!(!is_new_enough("unknown"));
        assert!(!is_new_enough("3"));
        assert!(!is_new_enough("3.x"));
    }

    #[test]
    fn this_interpreter_is_new_enough() {
        let found = check_interpreter(&interpreter_to_probe()).expect("python3 must be present");
        assert!(is_new_enough(&found), "python3 on this box reports {found}");
    }

    #[test]
    fn a_missing_schema_is_an_unknown_schema_not_a_default_one() {
        let payload = serde_json::json!({"verdict": "PASS"});
        let error = schema_of(&payload).unwrap_err();
        assert!(
            matches!(error, GateError::UnknownSchema { ref found, .. } if found.contains("no schema key")),
            "got {error:?}"
        );
    }

    #[test]
    fn a_payload_from_a_newer_gate_is_refused() {
        let payload = serde_json::json!({"schema": "elohim.gate/2", "verdict": "PASS"});
        let error = schema_of(&payload).unwrap();
        assert_eq!(error, "elohim.gate/2");
        assert!(!KNOWN_SCHEMAS.contains(&error.as_str()));
    }

    #[test]
    fn a_failing_payload_still_summarizes_because_a_red_row_is_data() {
        let run = GateRun {
            payload: serde_json::json!({
                "schema": "elohim.gate/1",
                "verdict": "FAIL",
                "skills": [{
                    "schema": "elohim.gate/1",
                    "skill": "elohim",
                    "verdict": "FAIL",
                    "instrument_source": "bundled",
                    "timed_out": false,
                    "instrument_error": null,
                    "instrument_pin": {
                        "status": "DRIFT",
                        "expected_sha256": "a920cdd5dd51f73222000000000000000000000000000000000000000",
                    },
                    "facts": [
                        {"id": "a", "status": "verified"},
                        {"id": "b", "status": "drifted"},
                    ],
                    "traps": [{"pass": true}, {"pass": false}],
                }],
                "summary": {
                    "skills": 1, "passed": 0, "failed": 1, "unlocated": 0,
                    "facts": 2, "facts_verified": 1, "facts_drifted": 1,
                    "traps": 2, "traps_holding": 1, "hygiene_findings": 0,
                    "unbound_claims": 0, "timed_out": 0, "runtime_seconds": 3.5,
                },
            }),
            exit_code: 1,
            stderr_tail: String::new(),
            command: "elohim --all --json".to_string(),
            schema: "elohim.gate/1".to_string(),
        };
        let summary = summarize(&run).expect("a FAIL payload is still a payload");
        assert_eq!(summary.verdict, "FAIL");
        assert_eq!(summary.exit_code, 1);
        assert_eq!(summary.rows.len(), 1);
        let row = &summary.rows[0];
        assert_eq!(row.verdict, "FAIL");
        assert_eq!(row.pin_status, "DRIFT");
        assert_eq!((row.facts_verified, row.facts_total), (1, 2));
        assert_eq!((row.traps_holding, row.traps_total), (1, 2));
        assert_eq!(row.pin_sha.len(), 12, "the pin is shown shortened, never in full");
    }

    #[test]
    fn an_unmeasured_skill_renders_its_reason_instead_of_blank_cells() {
        // The payload for a skill whose instrument never ran carries empty fact
        // lists. A table that renders that as "0/0, all fine" is the exact lie
        // this project exists to prevent, so the row must say why.
        let run = GateRun {
            payload: serde_json::json!({
                "schema": "elohim.gate/1",
                "verdict": "FAIL",
                "skills": [{
                    "skill": "tolerance-prover",
                    "verdict": "FAIL",
                    "instrument_source": "missing",
                    "timed_out": false,
                    "instrument_error": "no ledger at /tmp/x/ledger.json",
                    "instrument_pin": {"status": "UNLOCATED"},
                    "facts": [],
                    "traps": [],
                }],
                "summary": {"skills": 1, "failed": 1, "unlocated": 1},
            }),
            exit_code: 2,
            stderr_tail: String::new(),
            command: "elohim --all --json".to_string(),
            schema: "elohim.gate/1".to_string(),
        };
        let summary = summarize(&run).expect("an unmeasured payload is still a payload");
        let row = &summary.rows[0];
        assert_eq!(row.verdict, "FAIL");
        assert_eq!(row.pin_status, "UNLOCATED");
        assert_eq!(row.instrument_source, "missing");
        assert!(
            row.problem.contains("no ledger at"),
            "the reason must survive into the row, got {:?}",
            row.problem
        );
    }

    #[test]
    fn a_missing_summary_is_reported_rather_than_defaulted_to_zero() {
        let run = GateRun {
            payload: serde_json::json!({"schema": "elohim.gate/1", "skills": []}),
            exit_code: 0,
            stderr_tail: String::new(),
            command: "elohim --all --json".to_string(),
            schema: "elohim.gate/1".to_string(),
        };
        let error = summarize(&run).unwrap_err();
        assert!(
            matches!(error, GateError::Malformed { ref detail } if detail.contains("summary")),
            "got {error:?}"
        );
    }

    #[test]
    fn command_splitting_keeps_a_quoted_path_usable() {
        let (program, args) = split_command("python3 /tmp/a b/harness_run.py --all");
        assert_eq!(program, "python3");
        assert_eq!(args, vec!["/tmp/a".to_string(), "b/harness_run.py".to_string(), "--all".to_string()]);
    }

    #[test]
    fn a_missing_program_is_named_rather_than_reported_as_a_spawn_failure() {
        let error = run_gate("/nonexistent/definitely-not-here").unwrap_err();
        assert!(
            matches!(error, GateError::Spawn { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn the_viewers_flags_land_after_an_interpreter_and_script() {
        // The regression that cost an end-to-end run: `--all --json` was inserted
        // after the program, so `python3 /path/harness_run.py` became
        // `python3 --all --json /path/harness_run.py` and python3 reported
        // "Unknown option: --all". The gate never ran, and the app reported it
        // as a broken install rather than as the mistake it was.
        let run = run_gate("python3 /tmp/nonexistent-harness.py").unwrap_err();
        // No payload, because the script does not exist -- but the program was
        // started, not the flags rejected, which is what the fix buys.
        assert!(
            matches!(run, GateError::NoPayload { .. } | GateError::Spawn { .. }),
            "got {run:?}"
        );
    }

    #[test]
    fn the_viewers_flags_land_after_the_overrides_own_arguments() {
        // `echo` returns what it was given, so the returned bytes are the argv.
        // A caller's own flags must survive, ahead of the two the viewer adds --
        // otherwise a developer cannot pass `--fail-under` without having to
        // restate what the viewer already asks for.
        let error = run_gate("echo mine").unwrap_err();
        match error {
            GateError::NotJson { stdout_head, .. } => {
                assert_eq!(stdout_head.trim_end(), "mine --all --json");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn output_that_is_not_json_is_refused_with_the_first_bytes_shown() {
        // `echo` returns its arguments, so this pins the argv the viewer adds.
        // A consumer that dropped `--all --json` would gate one skill and report
        // the whole tree green, which is the defect this project keeps producing.
        let error = run_gate("echo").unwrap_err();
        match error {
            GateError::NotJson {
                stdout_head, detail, ..
            } => {
                assert_eq!(stdout_head.trim_end(), "--all --json");
                assert!(!detail.is_empty(), "the parse error must be carried through");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn empty_output_is_a_broken_install_not_a_failing_gate() {
        // The distinction the user has to act on: exit 1 with a payload is a
        // red gate, exit 1 with nothing is a broken install.
        let error = run_gate("true").unwrap_err();
        match error {
            GateError::NoPayload { stderr_tail, .. } => assert!(stderr_tail.is_empty()),
            other => panic!("got {other:?}"),
        }
    }
}