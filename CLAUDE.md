# Wixen Chat

A fully accessible, cross-platform chat client built in Rust, primarily implementing the
Matrix specification. Other protocols may follow based on demand. Accessibility is the
core mission, not a feature: the client must be first-class for screen reader users on
Windows (NVDA, JAWS, Narrator), macOS (VoiceOver), and Linux (Orca).

<!-- BEGIN GUARDRAILS (managed by /setup-guardrails) -->
## Guardrails

Standing guardrails in `~/.claude/CLAUDE.md` apply; below is this project's tightening.

### Code

- **Red/green TDD, always.** Invoke the `tdd` skill for every implementation, bug fix, or
  feature. Test command: `cargo test` (workspace: `cargo test --workspace`). Write the
  failing test first, minimum code to pass, then refactor. No production code without a
  failing test that demanded it.
- **Elegant code.** Invoke the `elegant-code` skill. Rust idioms: `Result` + `thiserror`
  for library errors, no `unwrap`/`expect` outside tests, exhaustive matches over
  catch-all arms, newtypes over primitive obsession, small modules, `#[must_use]` where
  ignoring a value is a bug. `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo fmt --check` must pass before any commit.
- **Done means it runs.** A feature is complete only when a non-test path reaches it end
  to end. Run `dead-code-hunter` after features; no stubs presented as complete — gate
  and document unfinished boundaries instead.

### Accessibility (WCAG 2.2 AA — core mission)

Serve all disability categories, not just screen reader users:

- **Blind:** correct platform accessibility API exposure (UIA on Windows, NSAccessibility
  on macOS, AT-SPI on Linux) — name/role/value/state on every control; managed focus;
  announced dynamic changes (incoming messages, typing indicators, presence) with
  bounded, non-flooding announcements; sensible reading order.
- **Low vision:** contrast ≥ 4.5:1 text / 3:1 UI and graphics; never color alone
  (presence, unread, mentions each need a non-color cue); visible focus indicator;
  respect system text scaling and high contrast themes.
- **Motor:** full keyboard operability; platform-standard accelerators; no drag-only
  interactions; no timing traps; targets ≥ 24×24 px.
- **Cognitive:** plain language; predictable, consistent navigation; clear error
  recovery; no redundant entry (3.3.7); accessible authentication (3.3.8) — SSO/QR
  login paths must not require transcription or memory tests.
- **Hearing:** visual equivalent for every audio cue (message sounds, calls, alerts);
  captions/transcripts for any audio or video content surfaced by the client.
- **Vestibular/photosensitivity:** honor reduced-motion preferences; nothing flashing
  more than three times per second.

Verify with real assistive technology (NVDA at minimum on Windows), not just automated
checks — structure present is not experience good. Use the accessibility specialist
agents (Desktop Accessibility Specialist, Desktop A11y Testing Coach) for UI work.
Automated scans (Axe.Windows on the UIA tree) catch roughly half of WCAG; they do not
replace assistive-tech testing. Wire an Axe.Windows CI job once the app has a window to
scan; until then, every merged feature that emits user-facing output needs a test
asserting a text equivalent for any color or audio cue.

### Docs and writing

- User-facing docs, README, guides: `writing-craft` skill. Plain language, no em-dashes,
  no AI-slop vocabulary.
- Commits, PR descriptions, reviews: `writing-style` skill. Direct, why over what.

### Project-specific rules

- **No AI attribution, ever.** Commits, branches, PRs, and code comments carry only
  Pratik's identity (PratikP1). Never add Co-Authored-By, "Generated with", or any
  AI/Claude reference to git history or code.
- **Pure Rust.** No non-Rust runtime components. Build scripts and CI glue may use the
  platform shell; the shipped product is Rust only.
- **Dependency discipline.** Run the `dependency-audit` skill before any `cargo add`.
  Matrix protocol support should build on the `matrix-sdk` crate family unless an audit
  concludes otherwise; record significant dependency decisions in `docs/decisions/`.
- **Cross-platform:** Windows, macOS, Linux are all first-class targets. CI tests all
  three; platform-specific code lives behind cfg-gated modules with a common trait
  boundary.
<!-- END GUARDRAILS -->
