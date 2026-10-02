//! Tauri shell for the elohim gate viewer.
//!
//! Every decision lives in `gate`; this file only moves values across the
//! boundary. The commands return a tagged result rather than throwing, because
//! the failure modes here are things the user is meant to read -- no gate, no
//! Python, a payload from a newer contract -- not exceptions to be logged.

mod gate;

use gate::{GateError, GateSummary};

/// What the frontend receives: either a summary to render, or a reason it cannot
/// be rendered. Serialized as an externally-tagged enum so the frontend switches
/// on `status` and cannot accidentally read a summary out of an error.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum GateResponse {
    Ready { summary: GateSummary },
    Unavailable { message: String },
}

impl From<Result<GateSummary, GateError>> for GateResponse {
    fn from(result: Result<GateSummary, GateError>) -> Self {
        match result {
            Ok(summary) => GateResponse::Ready { summary },
            Err(error) => GateResponse::Unavailable {
                message: error.to_string(),
            },
        }
    }
}

/// Report whether this machine can run the gate, and why not if it cannot.
///
/// Called at startup rather than on first click, because a viewer that opens to
/// an empty window is a viewer the user has to guess about.
#[tauri::command]
fn preflight() -> Result<serde_json::Value, String> {
    let interpreter = gate::interpreter_to_probe();
    let python = gate::check_interpreter(&interpreter).map_err(|error| error.to_string())?;
    let command = gate::resolve_gate_command(std::env::var(gate::OVERRIDE_ENV).ok().as_deref())
        .map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "python": python,
        "gate_command": command,
        "schemas": gate::KNOWN_SCHEMAS,
    }))
}

/// Run the gate and return the summary to render.
///
/// Async, and the work is moved onto a blocking thread. A Tauri command that is
/// not `async` runs on the main thread, which is also the thread driving the
/// event loop -- so spawning the gate there froze the whole UI for the gate's
/// entire runtime, and on a loaded machine the window stopped answering before
/// the result ever arrived. The gate is a process with `wait()` on it; that
/// belongs off the event loop.
#[tauri::command]
async fn run_gate() -> GateResponse {
    match tauri::async_runtime::spawn_blocking(run_gate_blocking).await {
        Ok(response) => response,
        Err(error) => GateResponse::Unavailable {
            message: format!("The gate could not be run: {error}"),
        },
    }
}

/// Record what the window is showing, so the frontend's state is observable
/// without a screenshot.
///
/// The backend already logs its own half of every run. That is not enough: a
/// gate that finished and a window that painted the result are separate claims,
/// and on this machine only one of them can be read from a log. When the window
/// was observed stuck on "Running the gate" long after the backend had logged a
/// completed PASS, there was no way to tell from the log alone whether the
/// response had reached the JavaScript at all. This closes that gap: the
/// frontend reports each render, and the log shows whether the round trip
/// completed.
///
/// Deliberately a log and not a store. Nothing here is ever read back, so
/// keeping it would be state that exists only to be believed.
#[tauri::command]
fn report(state: String) {
    eprintln!("ui: {state}");
}

/// The blocking half of `run_gate`, kept separate so the command stays a
/// one-line move onto a worker thread.
fn run_gate_blocking() -> GateResponse {
    let command = match std::env::var(gate::OVERRIDE_ENV) {
        Ok(raw) => gate::resolve_gate_command(Some(&raw)),
        Err(_) => gate::resolve_gate_command(None),
    };
    let command = match command {
        Ok(command) => command,
        Err(error) => return GateResponse::Unavailable {
            message: error.to_string(),
        },
    };
    match gate::run_gate(&command).and_then(|run| gate::summarize(&run)) {
        Ok(summary) => {
            // One line to stderr, unconditionally. The gate's verdict is the one
            // number this app exists to report, and a GUI that renders it only
            // on screen cannot be checked by anything -- not a test, not a
            // screenshot script, not a user reading a log after the fact.
            eprintln!(
                "gate-run: verdict={} exit={} skills={} passed={} failed={} unlocated={} \
                 facts={}/{} traps={}/{} runtime={:.1}s",
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
                summary.runtime_seconds,
            );
            GateResponse::Ready { summary }
        }
        Err(error) => {
            eprintln!("gate-run: unavailable: {error}");
            GateResponse::Unavailable {
                message: error.to_string(),
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // No opener plugin. It was in the scaffold and nothing calls it; leaving it
    // registered would hand the app the ability to open arbitrary URLs and
    // filesystem paths for no benefit. The only thing this app does is read a
    // JSON payload and draw a table.
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![preflight, run_gate, report])
        .on_window_event(|window, event| {
            // Something on a shared desktop closes windows this app never asked
            // to close: a CloseRequested arrives from outside, with no matching
            // request anywhere in this code, and the app then exits cleanly with
            // nothing on stderr. Measured, not assumed -- suppressing it keeps the
            // window up indefinitely.
            //
            // Opt-in and off by default, because it is a workaround for the
            // environment rather than for this program: with it on, the window
            // ignores a close the user asked for, which would be wrong on a
            // desktop where nothing else is interfering.
            if std::env::var_os("ELOHIM_VIEWER_HOLD_OPEN").is_none() {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                eprintln!(
                    "ELOHIM_VIEWER_HOLD_OPEN is set; refusing an externally-sent close"
                );
                api.prevent_close();
                let _ = window;
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Seams the integration tests need, in one place.
///
/// `gate::run_gate` can only get a payload by spawning a process, so testing it
/// against captured payloads needs a way to inject one. Keeping that here rather
/// than making the functions more conditional means the tests exercise the same
/// `summarize` the app calls.
pub mod testing {
    use crate::gate::{self, GateError, GateRun};

    pub use crate::gate::{GateSummary, SkillRow};

    /// Build a `GateRun` from a captured payload in `tests/fixtures/`.
    ///
    /// `exit_code` is passed in rather than read, because the gate's exit code is
    /// not in the payload: it is the process's, and a fixture records output
    /// only. The real exit code for each fixture is recorded in
    /// `fixtures/README.md` and asserted in the test that uses it.
    pub fn run_from_json(name: &str, exit_code: i32) -> GateRun {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("fixture {name} is missing: {error}"));
        let payload: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|error| panic!("fixture {name} is not JSON: {error}"));
        let schema = gate::schema_of(&payload)
            .unwrap_or_else(|error| panic!("fixture {name} is unreadable: {error}"));
        GateRun {
            payload,
            exit_code,
            stderr_tail: String::new(),
            command: "elohim --all --json".to_string(),
            schema,
        }
    }

    pub fn summarize_payload(run: GateRun) -> Result<GateSummary, GateError> {
        gate::summarize(&run)
    }

    /// Spawn a real gate and summarize it, by the same two steps the app takes.
    ///
    /// Exposed so the end-to-end test proves the app's own path -- argv
    /// construction, process start, stdout parse, summarize -- rather than a
    /// reimplementation of it.
    pub fn run_gate_command(command: &str) -> Result<GateRun, GateError> {
        gate::run_gate(command)
    }

    pub fn summarize(run: &GateRun) -> Result<GateSummary, GateError> {
        gate::summarize(run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_becomes_a_readable_message_not_an_exception() {
        let response: GateResponse = Err(GateError::NotFound {
            override_var: gate::OVERRIDE_ENV,
            path_entry: "/usr/bin".to_string(),
        })
        .into();
        match response {
            GateResponse::Unavailable { message } => {
                assert!(message.contains("pip install elohim"), "got {message}");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn a_ready_response_carries_the_schema_it_read() {
        // The frontend is told which contract produced these numbers, so it can
        // say so on screen rather than implying the numbers came from nowhere.
        let json = serde_json::to_string(&GateResponse::Ready {
            summary: GateSummary {
                schema: "elohim.gate/1".to_string(),
                verdict: "PASS".to_string(),
                exit_code: 0,
                skills: 0,
                passed: 0,
                failed: 0,
                unlocated: 0,
                facts_verified: 0,
                facts: 0,
                facts_drifted: 0,
                traps_holding: 0,
                traps: 0,
                hygiene_findings: 0,
                unbound_claims: 0,
                timed_out: 0,
                runtime_seconds: 0.0,
                rows: vec![],
                stderr_tail: String::new(),
                command: "elohim --all --json".to_string(),
            },
        })
        .expect("serializes");
        assert!(json.contains("\"status\":\"ready\""), "got {json}");
        assert!(json.contains("\"schema\":\"elohim.gate/1\""), "got {json}");
    }
}