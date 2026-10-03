# Changelog

Every release of `elohim-gate-viewer`, newest first. Nothing here is published
automatically: a version number in a manifest is not a release, and this file
exists to keep that difference visible.

Two rules, both because the alternative was tried here first:

- **No commit shas.** A sha in a document that outlives the commit naming it goes
  stale the moment history is rebased, after which it describes a tree that no
  longer exists. Each entry instead says how to re-derive what it claims.
- **Every figure names the command that produces it.** A number nobody can
  reproduce is not a fact.

There are no comparison links at the foot of this file because this repository
has never been tagged; there would be nothing for one to compare.

## [Unreleased]

Nothing yet.

## [0.1.0]

**Not tagged. Not published.** No tag has ever been created in this repository
and no GitHub release exists, so this heading declares a version rather than
describing an event. It covers the whole history as it stood when it was
written, which `git log --oneline` lists in full.

### Added

**The app.** A Tauri v2 desktop viewer that runs the elohim gate as a local
process and renders its `elohim.gate/1` payload: a verdict, a per-skill table,
and the reason beside anything that failed. It draws the gate's numbers; it does
not recompute them. It lives in its own repository so that it cannot reach the
gate through a relative path — a viewer inside elohim could go green by agreeing
with the repository it lives in, which is the failure this project keeps
producing.

It refuses five things, each because showing something would be worse than
showing nothing: a payload whose `schema` it does not know, or that has no
`schema` key at all; an unparseable Python version treated as a supported
interpreter; a gate that exits non-zero having printed nothing, which is a broken
install rather than a red gate, reported as unavailable; an unmeasured skill
rendered as a clean one, where the reason is shown instead; and any host path
carried into the frontend — the pin's sha256, shortened to 12 characters, is
what travels.

**The static report.** A hosted page cannot run the gate: it needs local Python
3.10+ and the repository it measures. So this is not a service. It renders one
payload that the publisher generated from their own run, and nothing else — no
backend, no input, no telemetry.

Two halves, kept separate on purpose. `tools/publish_report.py` is the only thing
that turns a payload into a publishable one: stdlib-only, and it refuses rather
than guesses — exit 2 if the schema is not `elohim.gate/*`, exit 1 if any rooted
path survives into the output. `src/report/guard.ts` is the second half and it
scrubs nothing. Handed a payload containing a local path, it renders "this
report was not published" and lists the offending JSON paths instead of the
report, so an unsanitised payload fails closed in the browser rather than
relying on the publisher having run.

Redaction is field-aware because a single regex cannot do it. A path-valued key
gets `basename()` applied to the whole value; every other string gets a sweep
for rooted paths as a net. The first version only swept, and turned a real home
directory into `My Docs/instrument.py` — the directory survived, and the residual
check missed it because `/Docs/` is not one of the known prefixes.

**Continuous integration.** Two workflows, deliberately not one.

- `pages.yml` deploys `dist/`. Before building, it runs the 12 scrubber tests and
  greps `public/data/` for 17 rooted-path prefixes, so a payload that still
  carries a local path cannot reach the live site. Deploys are queued rather than
  cancelled: a cancelled deploy leaves the site serving the previous build, which
  is worse than being a minute behind.
- `rust.yml` runs `cargo test` for `src-tauri` on every push to main, on every
  pull request, and on demand, with no paths filter. Before it existed nothing
  compiled `src-tauri` on any push: its 23 tests were run by hand and the result
  was recorded nowhere a reader could check.

### Fixed

**Two bugs that only getting the window onto a screen could find.**

*The gate ran on the UI thread.* `run_gate` was a plain `#[tauri::command]`,
which runs on the main thread — the same thread that drives the event loop — so
a fifteen-second gate froze the entire window and it stopped answering before the
result arrived. It is now `async` and delegates to `spawn_blocking`.

*The real-gate test was a false pass.* It returned early when `ELOHIM_GATE_CMD`
was unset and cargo counted that as a pass, so the only test covering the
end-to-end link printed green while executing nothing. It is now `#[ignore]`d
with a reason, so cargo reports it as ignored, and `npm run test:gate` runs it.

**Provenance, after the screenshots proved the argument wrong.** The window's
provenance line printed the gate command verbatim, and the README argued that
this was deliberate: a verdict with no idea which checkout produced it is the
failure this app exists to make visible. That argument was wrong about which
half matters, and the screenshots settled it — text around a PNG cannot
un-publish the PNG. `reduceCommand()` now reduces any rooted token to its
basename, so the header reads `harness_run.py --all --json`: which command, not
whose laptop. Splitting is on whitespace, so a path containing a space is cut in
half, which is accepted because a truncated path beats publishing a home
directory.

**Both screenshots are reproducible now; they were not before.** The window needs
a gate that passes, and today's tree does not pass. `tools/emit_fixture.py`
prints a checked-in fixture instead, tolerates the `--all --json` flags
`gate.rs` appends, validates the schema before printing, and mirrors the gate's
exit code, so the failing shot still shows exit 1 as it always claimed to. The
capture driver now checks absolute width as well as aspect ratio: one crop came
back at 932x1016 against a committed 1882x2060 because another window covered
the left half, and the ratio test passed it — 0.917 against 0.914 is not a
difference any ratio assertion can see.

**The machine's home directory stopped being a published artifact.** Two steps,
text then images. The publisher and its tests used the author's real home path as
the worked example for a path containing a space; replaced with `/home/example`,
which reads as the placeholder it is. The screenshots that had rendered it were
re-taken rather than patched, because they are the evidence and the evidence had
to be re-earned. A Cargo comment naming the directory that prompted it was
dropped for the same reason: that is this machine's layout, not a fact about
Cargo.

**Two prose corrections, both of the same shape.** The README carried the same
two paragraphs twice, and the pages workflow's header pointed at "the release
plan for the CI gap that leaves" — a plan in another repository, about a gap
that `rust.yml` has since closed, so a reader who followed the reference found
nothing. The same workflow also claimed the Python tests were "not here because
this workflow has no reason to be the place they run", which was wrong in a way
worth correcting rather than leaving: `tools/test_publish_report.py` does run
here, as a blocking step before the build, and it is the only Python suite in
the repository.

**An "unexplained" note that turned out to have an explanation.** The extra gate
run during screenshotting was replaced by the measurement that resolved it: a
trusted single-click input event (`isTrusted=true`, `detail=1`) on the Run
button, because the capture harness focuses the window on every attempt and
something else on that shared desktop clicked where the button was. The app is
correct, since a clicked button re-runs the gate — and the harness tolerates it,
because a restarted run puts the window back into its in-progress state, which
fails the accent-pixel check and is retried.

### Known limitations

**This repository has never been tagged or published.** Both manifests say
`0.1.0`; `git tag -l` is empty and `git describe` fails. The heading above is a
declaration.

**The leaked username is still reachable in git history.** Two committed
screenshots rendered the author's home directory in their header. Both were
re-taken and the working tree is clean, but the old blobs remain in history, and
the history of `shots/` is the record. Un-publishing them requires rewriting
published history, which is refused without an explicit decision from a person.

**The leak gate cannot see images, and that is how the leak survived.** The gate
is `git grep -nI`, and `-I` skips binaries — correctly, since a PNG is compressed.
It is not a hypothetical blind spot: it is the reason a repository-wide gate
added by the same two commits that fixed the text could not catch the images. The
path is rendered into pixels, so no grep over any revision can find it.

**The `glib` advisory is open and not fixable from here.**
`GHSA-wrw7-89jp-8q8g`, moderate, unsound `Iterator`/`DoubleEndedIterator` impls
for `glib::VariantStrIter`, fixed in `glib` 0.20.0. This build resolves `glib`
0.18.5, through `gtk` 0.18.2 and `tauri` 2.12.1 — all three read from
`src-tauri/Cargo.lock`, not from memory. `cargo update -p glib` reports zero
packages available, because `glib` 0.18 is what the whole GTK stack of the
current Tauri ships, and the first release carrying `glib` 0.20 is
`tauri` 3.0.0-alpha.4. Taking an alpha framework to clear a moderate advisory in
a transitive binding is the worse trade, so it stays open and is written down.
Exposure is small but not zero: the unsound code is a Rust-side iterator over GLib
variants, and nothing in this app references `glib` or any `Variant` type.

**The Rust workflow has never run on a runner.** Every `cargo` step in it was
executed by hand, at the command level, on the tree that added it. The
`apt-get install` step has not, because this machine already has Tauri's Linux
build dependencies — so whether those eight package names resolve on a clean
runner is untested. They were copied verbatim from Tauri's v2 prerequisites
rather than assembled from memory, because an unresolvable name fails the job
with "Unable to locate package", which names neither the package nor the file.
The first push settles it either way.

**That workflow had a defect on its first draft, and only measurement found it.**
Its two `cargo` steps reached for the crate differently, and the one written
without a directory reached the wrong manifest: there is no `Cargo.toml` at the
repository root, so cargo walked up the filesystem and found an unrelated Cargo
workspace two directories above this checkout, whose members are three crates
this repository has nothing to do with. Both steps now name the crate with
`--manifest-path`. Committed as first written, it would have failed on a runner
while naming a project nobody here has heard of. `src-tauri/Cargo.toml` already
declares its own `[workspace]`, which stops the ancestor workspace absorbing
this crate — but that protection runs one way, and it does not stop a bare
`cargo` from the root being absorbed.

**No distributable is ever built.** No AppImage, no `.deb`, no `tauri build`, no
code signing. The dependencies the workflow installs are the ones the build
script links against, not the ones a distributable needs, so a green run says the
logic compiles and the tests pass — and nothing about whether a release artifact
would install.

**The window has only been seen on a machine with no usable GPU.** Software
rendering — `LIBGL_ALWAYS_SOFTWARE=1`, `GSK_RENDERER=cairo`,
`WEBKIT_DISABLE_COMPOSITING_MODE=1` — is what makes WebKit allocate a render
surface here; without it the failure surfaces as a Wayland protocol error, which
reads like a compositor problem and is not one. `GDK_BACKEND=x11` is not the fix
and was measured to make the app exit silently with status 0 after about four
seconds. Nobody has run this app on a machine that has a GPU.

**`src-tauri/Cargo.toml` still carries its scaffolding metadata:**
`description = "A Tauri App"` and `authors = ["you"]`. Left alone deliberately.
Filling them in is a claim about who ships this, and nobody has decided that.

**One environment caveat, not a defect in this app.** On the capture machine,
something outside the app sends the window a close request a few seconds after it
maps: `CloseRequested` → `Destroyed` → `ExitRequested`, with no matching request
anywhere in this code. `ELOHIM_VIEWER_HOLD_OPEN=1` suppresses it for the
screenshot harness. It is off by default and should stay off — on a normal
desktop it would make the window ignore a close the user actually asked for.

### Out of scope

Not in this file because it is not this repository's work: elohim's own releases,
the seven skills, and the gate itself. This repository consumes `elohim.gate/1`
and renders it.

### Re-deriving this entry

| Group | Commits |
| --- | ---: |
| The app as first written | 1 |
| Getting the window onto a screen, and diagnosing what moved it | 2 |
| The static report and its deploy | 1 |
| Provenance and home-directory hygiene | 3 |
| The dependency advisory and core dumps | 1 |
| Prose corrections | 2 |
| Rust in CI | 1 |
| **Total** | **11** |

Check it two ways, and the second is the one that matters:

- `git rev-list --count HEAD` prints the total. It read 11 over the history as it
  stood one commit before the commit that wrote this file, which is how any
  release's own commit escapes its own table; it reads 12 now and will read more
  after anything is added. The table is a record of this release, not a live
  claim.
- `git log --oneline` prints the list. A subject no group accounts for is the
  gap. A count is not a check; the list is.

The figures quoted above, each with the command that produces it:

| Claim | Command | Reads |
| --- | --- | --- |
| 23 tests pass, 1 ignored | `cargo test --manifest-path src-tauri/Cargo.toml --locked` | `23 passed; 0 failed; 1 ignored` |
| 12 scrubber tests | `python3 tools/test_publish_report.py` | `Ran 12 tests` |
| 17 rooted-path prefixes | `sed -n 's/.*\/\((home[^/]*)\)\/.*/\1/p' .github/workflows/pages.yml \| tr '\|' '\n' \| grep -c .` | 17 |
| The report is 8 files | `PAGES=1 npm run build && find dist -type f \| wc -l` | 8 |
| The Pages deploy is green | `gh run list --workflow pages.yml` | 2 runs, both successful, at the time of writing |
| No tag exists | `git tag -l` | empty |
| `glib` 0.18.5, `gtk` 0.18.2, `tauri` 2.12.1 | `src-tauri/Cargo.lock` | as quoted |