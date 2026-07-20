# Wixen Chat: Foundation Design and Implementation Plan

## Overview

Wixen Chat is a fully accessible, cross-platform Matrix chat client in pure Rust.
This plan covers the foundation: the guiding principles, the architecture, the crate
choices, the accessibility design, the gaps in the proposed stack, and the
implementation phases from empty scaffold to a usable beta.

The stack the project commits to, pending the Phase 0 spikes:

| Concern | Crate | Version | Why |
|---|---|---|---|
| UI toolkit | `wxdragon` | 0.9.17 | Native Win32/Cocoa/GTK controls, so the platform accessibility tree comes from the OS, not a reimplementation |
| Announcements | `accesskit` + platform adapters | 0.24.1 | Live regions (Polite/Assertive) for speech that has no widget to live in |
| Matrix protocol | `matrix-sdk`, `matrix-sdk-ui` | 0.18.0 | Maintained by matrix.org, powers Element X; E2EE, sliding sync, timeline API |
| Async runtime | `tokio` | current | Required by matrix-sdk |
| Secrets | `keyring` | current | OS credential store for tokens and store passphrases |
| Config | `serde` + `toml` | current | Same pattern as Wixen Terminal |
| Errors | `thiserror` | current | Typed errors in library crates |
| Logging | `tracing` | current | Structured, no println |
| Audio cues | `rodio` | current | Every cue paired with a visual and announced equivalent |
| Link opening | `opener` | current | With a scheme allowlist; Wixen Terminal shipped a hole here once |

## The Four Questions

Wixen Terminal is judged by four questions. Wixen Chat adopts the same frame.
Ask them of every change.

### What is it for?

Making real-time conversation legible to people who cannot see it. Chat is where
work, community, and friendship happen, and the mainstream clients are web apps or
Electron wrappers where a screen reader user tabs through unlabeled regions, loses
their place on every new message, and hears either everything or nothing. Wixen Chat
treats conversation as structured data: rooms, messages, senders, replies, edits,
reactions, membership, presence, and read state are declared to the platform
accessibility API through native controls, and everything that happens outside the
focused widget reaches the user through a deliberate, bounded announcement channel.
A blind user should hold their own in a fast-moving room, not reconstruct it
afterward.

### What does it strengthen?

The independence of its users: conversation without a sighted intermediary and
without a degraded second-class interface. The Matrix ecosystem: an open,
federated protocol deserves a client whose accessibility matches the openness of
its transport, and Matrix gains users that the existing clients turn away. And the
same structural principle as Wixen Terminal: the application declares its meaning.
The client says "new message from Alice in Rust Users, mentions you" as structured
fact; the screen reader is never left to guess it from pixels or DOM scraping.

### What does it replace?

For its user, it replaces Element and the other Electron and web clients: usable
with a screen reader in the technical sense, painful in the practical one. It does
not replace the screen reader, which owns review, verbosity preferences, and speech;
Wixen Chat owns what the application should expose and announce. It does not
replace the homeserver or bridges; it is a client. And it does not replace Wixen
Terminal or Terminal Access; those serve the command line, this serves
conversation. Each tool does one thing excellently.

### What does it allow to be done poorly?

This question generates the guardrails, because every strength here has a failure
mode that looks like success:

- **Announcement flooding.** A client that can speak every event can bury a user in
  a busy room until they mute it and miss what mattered. Announcements must be
  prioritized, coalesced, deduplicated, and bounded, and the bounds must be tested.
- **Structure present, experience poor.** Native widgets give us a tree for free,
  and a tree is not an experience. Focus that jumps on refresh, a timeline that
  re-announces itself, an unlabeled button: each passes automated checks. Only a
  real screen reader run proves the experience. (Wixen Terminal shipped a COM bug
  that froze NVDA and passed every test.)
- **Security theater around E2EE.** Encryption UX done sloppily trains users to
  click through verification warnings. Verification flows must be fully accessible
  (SAS emoji presented by name, never image alone) and honest about state.
- **Feature sprawl.** "Other protocols based on demand" is an invitation to breadth
  over excellence. Matrix support ships excellent before any second protocol is
  considered, and the protocol boundary stays a trait so breadth never rots the core.
- **Absorbing upstream failures.** When a sender posts an image with no alt text or
  a client sends broken formatting, Wixen Chat says so plainly rather than papering
  over it. A better ecosystem needs the gap visible.
- **Privacy leaks through speech.** A client that announces message content will
  speak private messages to a room of people if the user is presenting or away from
  headphones. Content announcement needs per-room and global controls and a fast
  global mute.

## Architecture

### Workspace layout

The single package becomes a Cargo workspace. The root binary stays `wixen-chat`.

```
Cargo.toml            # workspace + root binary
src/main.rs           # wiring only: runtime, UI, bridge
crates/
  wixen-chat-core/    # protocol-agnostic domain: rooms, messages, events,
                      # announcement policy, verbosity settings. No I/O, no UI.
  wixen-chat-matrix/  # matrix-sdk integration: session, sync, timeline mapping
                      # into core types. Implements core's ChatProtocol trait.
  wixen-chat-announce/# the announcement channel: queue, priorities, coalescing,
                      # flood bounds, and the AccessKit live-region surface
  wixen-chat-ui/      # wxdragon: windows, room list, timeline, composer, dialogs
  wixen-chat-config/  # TOML config, settings persistence, schema
```

Rationale: core and announce are pure logic and get the deepest test coverage;
matrix and ui are integration shells kept as thin as possible. The `ChatProtocol`
trait boundary in core is what "other protocols based on demand" will use later, and
until then it has exactly one implementation.

### Threading model

- UI thread: the wx event loop. All wxdragon calls happen here, no exceptions.
- A tokio multi-thread runtime on background threads owns matrix-sdk.
- Backend to UI: updates cross via a channel drained with wxdragon's `call_after`
  main-thread dispatch (verified to exist; the Phase 0 spike proves it under load).
- UI to backend: commands (send message, join room) go over a tokio mpsc channel
  to the runtime.
- No shared mutable state across the boundary. Messages are owned values of core
  types.

### The announcement channel (AccessKit's role)

The accessibility tree covers what has a widget. Chat constantly produces speech
that has no widget: a message arriving in an unfocused room, a typing indicator,
a connection drop, a send failure, a mention while you are in another window.

Design:

1. **Policy engine** (`wixen-chat-announce`, pure logic): every event enters as an
   `AnnouncementRequest { text, priority, room, kind }`. The engine applies user
   verbosity settings (per event kind, per room), deduplicates, coalesces bursts
   ("14 new messages in Rust Users" instead of 14 announcements), and enforces a
   rate bound. Output is a small stream of `Announcement { text, urgency }`.
   This is where guardrail 4 (distinct and bounded) is enforced and tested.
2. **Delivery surface**: an AccessKit tree hosted on a dedicated hidden child
   window, containing a live-region node (`Live::Polite` or `Live::Assertive` per
   urgency). Updating the node's text raises the platform live-region event, which
   screen readers speak. AccessKit owns that child window's accessibility
   exclusively; wx native accessibility owns everything else. The two never share
   a window, which avoids fighting over WM_GETOBJECT on Windows.
3. **Fallback**: delivery is behind an `Announcer` trait. If the Phase 0 spike
   shows a platform where the AccessKit live region is not spoken, that platform
   gets a direct implementation (UIA `RaiseNotificationEvent` on Windows via the
   `windows` crate, `NSAccessibilityAnnouncementRequested` on macOS, AT-SPI
   announcement on Linux) without touching policy or callers.

### Accessibility design (beyond announcements)

- Room list: native list control, one item per room, name plus unread count plus
  mention state in the accessible name. Never color alone for unread.
- Timeline: native list control, one item per message; accessible name is
  "sender, content, time, state" with edits and replies declared in text.
  Virtualized only if the native control stays accessible when virtual (spike).
- Composer: native multiline text control. Standard keys. Enter sends,
  Shift+Enter for newline, both remappable.
- Full keyboard map documented and platform-conventional; every action reachable
  from the menu bar (discoverable by screen reader users), no drag-only anything.
- SAS device verification presents emoji by localized name in text, and the
  decimal fallback, never images alone.
- All sounds optional, all paired with a visual and an announceable equivalent.
- Reduced motion honored; no flashing content at all.
- Every phase ends with a real NVDA pass; VoiceOver and Orca before beta.

## What's missing (gap analysis)

Gaps in the proposed stack, with the plan's answer for each:

1. **The announcement channel is unproven.** AccessKit live regions next to wx
   native accessibility is the load-bearing novel idea and nobody has shipped it.
   Phase 0 gates the whole plan on this spike, with the direct platform-API
   fallback named above.
2. **wxdragon's accessibility coverage is undocumented.** Native controls should
   inherit platform accessibility, but wxWidgets uses generic (custom-drawn)
   implementations for some widgets on some platforms, and those are invisible or
   poor for screen readers. The spike tests the exact widgets this app needs
   (frame, menu bar, list, multiline text, dialogs) with NVDA, VoiceOver, and
   Orca before anything is built on them. Widget choices follow the results.
3. **VoIP and video calls.** matrix-sdk has no media stack; Element Call is a
   separate system. Out of scope for v1. Gated and documented, never stubbed.
4. **Local search in encrypted rooms.** No maintained Rust crate provides indexed
   E2EE-friendly message search. v1 ships server-side search for unencrypted
   rooms and states the limitation. A local index is a documented later phase.
5. **Rich message rendering.** Matrix messages carry an HTML subset. wx rich
   controls have weak accessibility. v1 renders to plain text preserving meaning
   (links enumerated and openable, code blocks and quotes declared in text),
   which is better for the target user than a pretty inaccessible view.
6. **Spell check.** No good cross-platform Rust story. Deferred; documented.
   Platform-native checkers (ISpellChecker, NSSpellChecker, enchant) are the
   likely later route.
7. **Notifications on Windows.** `notify-rust` is strongest on Linux/macOS; on
   Windows toast notifications may need `tauri-winrt-notification`. Decided in
   the notifications phase behind one trait.
8. **i18n.** Nothing in the stack does localization. v1 is English with all user
   strings behind a `fluent`-ready lookup so localization is a translation task,
   not a refactor. Emoji names for SAS come localized from the SDK.
9. **Sliding sync server dependency.** matrix-sdk-ui's sync service expects
   simplified sliding sync (MSC4186), now in Synapse but not universal. The spike
   verifies behavior against a non-supporting server; if degradation is poor, v1
   documents the server requirement.
10. **Media accessibility can't be conjured.** Incoming images without alt text
    are announced as exactly that (guardrail 7). Outgoing images prompt for alt
    text with skip allowed. Audio/video get no automatic transcripts; stated.
11. **Packaging and signing.** Installers (MSI, dmg, flatpak) and code signing are
    external work with real-world costs; listed under post-completion, not faked
    as a build script.

## Development Approach

- **Testing approach: TDD, red/green, always.** The `tdd` skill on every task.
  No production code without a failing test that demanded it.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo test --workspace` must pass before any task is complete.
- Every task includes its tests as separate checklist items, success and error
  paths both.
- A feature is done when a non-test path reaches it (guardrail 1), and an
  accessibility feature is done when a screen reader confirms it (guardrail 2).
- Update this plan as scope changes: `[x]` when done, ➕ for discovered tasks,
  ⚠️ for blockers.

## Testing Strategy

- **Unit tests**: the bulk, concentrated in `wixen-chat-core` and
  `wixen-chat-announce` (policy, coalescing, bounds, verbosity: all pure logic).
- **Integration tests**: `wixen-chat-matrix` against `matrix-sdk`'s test utilities
  and a mocked homeserver (`wiremock`), covering login, sync mapping, and send
  paths without network.
- **Property tests**: `proptest` on the announcement policy (no input sequence may
  exceed the rate bound or drop an assertive announcement) and on message-to-text
  rendering (no panic on arbitrary HTML subset input). Multibyte slicing bugs are
  a known recurring class in Wixen projects.
- **End-to-end accessibility**: scripted NVDA runs are not automatable in CI yet;
  each phase's exit criteria include a manual NVDA checklist, recorded in
  `docs/a11y-verification.md`. Once a window exists, an Axe.Windows scan job is
  added to CI as the automatable half.

## Implementation Steps

### Phase 0, Task 1: Spike: wxdragon accessibility census

**Files:**
- Create: `spikes/wx-a11y/` (throwaway crate, excluded from workspace)
- Create: `docs/spikes/20260720-wx-a11y.md` (findings)

- [ ] build a wxdragon app with the exact widgets the client needs: frame, menu
      bar with accelerators, list control, multiline text, button row, modal dialog
- [ ] verify `call_after` dispatch from a background thread under a 100-msg/s load
- [ ] NVDA pass on Windows: every control reports name/role/value; focus is sane;
      list updates do not steal focus or re-announce the world
- [ ] record VoiceOver and Orca results (or gate them with a date if hardware
      access delays them; do not skip silently)
- [ ] write findings including the widget allowlist and any generic-widget traps
- [ ] decision recorded in `docs/decisions/0001-ui-toolkit.md`

### Phase 0, Task 2: Spike: AccessKit live-region announcement channel

**Files:**
- Create: `spikes/announce/` (throwaway)
- Create: `docs/spikes/20260720-announce-channel.md`

- [ ] host an AccessKit tree with one `Live::Polite` and one `Live::Assertive`
      node on a hidden child window inside the wx spike app
- [ ] verify NVDA speaks polite updates without interrupting and assertive ones
      with interruption, while wx native accessibility keeps working untouched
- [ ] verify no WM_GETOBJECT contention: wx tree intact under AT tree walks
- [ ] test the failure mode: updates while a wx dialog is modal
- [ ] if the live region is not spoken on a platform, prototype the direct
      platform notification fallback and record which path each platform uses
- [ ] decision recorded in `docs/decisions/0002-announcement-channel.md`

### Phase 0, Task 3: Spike: matrix-sdk login and sync

**Files:**
- Create: `spikes/matrix-login/` (throwaway)
- Create: `docs/spikes/20260720-matrix-sdk.md`

- [ ] password login against matrix.org; token restore across restart
- [ ] sync service + room list + timeline diff stream printed to stdout
- [ ] verify behavior against a homeserver without simplified sliding sync
- [ ] measure binary size and build time cost of matrix-sdk with e2e-encryption,
      sqlite, sso-login, qrcode features
- [ ] decision recorded in `docs/decisions/0003-matrix-stack.md`

### Phase 1, Task 1: Workspace conversion

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/wixen-chat-core/`, `crates/wixen-chat-announce/`,
  `crates/wixen-chat-config/` (Cargo.toml + lib.rs each)

- [ ] convert to workspace keeping root `wixen-chat` binary and existing test green
- [ ] workspace-level lints: deny unwrap/expect in production code, warn unsafe
- [ ] CI updated to `--workspace`; all three platforms still green
- [ ] write a workspace smoke test (root binary still prints identity)
- [ ] run fmt, clippy, tests: must pass before next task

### Phase 1, Task 2: Core domain model

**Files:**
- Create: `crates/wixen-chat-core/src/{room,message,event,identity}.rs`

- [ ] TDD the core types: RoomId/Room, MessageId/Message (sender, body, timestamp,
      edit state, reply target, reactions), membership and presence events,
      connection state
- [ ] TDD `ChatProtocol` trait: the seam matrix (and any later protocol) implements
- [ ] TDD accessible rendering: `Message::to_accessible_text()` producing the
      "sender, content, time, state" form the timeline and announcements share
- [ ] property test: rendering never panics on arbitrary content
- [ ] run fmt, clippy, tests: must pass before next task

### Phase 1, Task 3: Announcement policy engine

**Files:**
- Create: `crates/wixen-chat-announce/src/{policy,queue,verbosity}.rs`

- [ ] TDD priorities: assertive (errors, mentions, verification prompts) vs polite
      (messages, presence) vs off, per user verbosity settings
- [ ] TDD coalescing: burst of N messages in one room within the window becomes one
      summary announcement; mentions never coalesced away
- [ ] TDD rate bound and dedup; TDD global mute
- [ ] property test: no input sequence exceeds the rate bound; no assertive
      announcement is ever dropped or reordered behind polite ones
- [ ] run fmt, clippy, tests: must pass before next task

### Phase 2, Task 1: Announcer delivery surface

**Files:**
- Create: `crates/wixen-chat-announce/src/{announcer,accesskit_surface}.rs`

- [ ] TDD the `Announcer` trait plus a test double capturing announcements
- [ ] implement the AccessKit live-region surface per the spike's decision doc
- [ ] wire platform fallback(s) the spike found necessary, behind the same trait
- [ ] tests: surface state transitions (tree updates, urgency mapping); manual
      NVDA verification recorded in docs/a11y-verification.md
- [ ] run fmt, clippy, tests: must pass before next task

### Phase 2, Task 2: Matrix session and auth

**Files:**
- Create: `crates/wixen-chat-matrix/` (session.rs, auth.rs)

- [ ] TDD against wiremock: password login, error taxonomy (bad creds, bad server,
      network down) with user-facing error text per error-message-craft
- [ ] token + store passphrase in OS keyring via `keyring`; never on disk in plain
- [ ] session restore on startup; logout wipes keyring entries
- [ ] SSO/OIDC login path (browser handoff), keeping 3.3.8: no transcription task
- [ ] run fmt, clippy, tests: must pass before next task

### Phase 2, Task 3: Sync into core types

**Files:**
- Create: `crates/wixen-chat-matrix/src/{sync,mapping}.rs`

- [ ] TDD mapping of matrix-sdk-ui timeline diffs and room list updates into core
      Room/Message/event values
- [ ] TDD announcement request generation from sync events (new message, mention,
      membership, connection state) feeding the policy engine
- [ ] integration test: scripted sync produces expected core updates and
      announcement requests, in order, no floods
- [ ] run fmt, clippy, tests: must pass before next task

### Phase 3, Task 1: UI shell

**Files:**
- Create: `crates/wixen-chat-ui/` (app.rs, main_window.rs, bridge.rs)
- Modify: `src/main.rs`

- [ ] main window: menu bar (full action coverage), room list, timeline area,
      composer, status bar, built only from the spike's widget allowlist
- [ ] the tokio↔wx bridge: command channel out, update channel in via call_after;
      test the bridge logic with the UI stubbed
- [ ] login dialog wired to Phase 2 auth; errors surfaced accessibly (focus moves
      to the error, error text is the specific taxonomy text)
- [ ] keyboard map documented in docs/keyboard.md as it is built
- [ ] manual NVDA pass recorded; run fmt, clippy, tests: must pass before next task

### Phase 3, Task 2: Room list and timeline, live

- [ ] room list bound to sync updates: name, unread, mention state in accessible
      name; activity never steals focus
- [ ] timeline bound to timeline diffs: stable focus position across updates,
      new-message insertion without re-announcement of existing items
- [ ] announcements flowing end to end: policy engine to Announcer while window
      unfocused or in another room
- [ ] read receipts sent on read; unread state updates
- [ ] manual NVDA pass under a genuinely busy room; bounds hold; run checks

### Phase 3, Task 3: Sending

- [ ] composer send path: optimistic local echo, declared pending state, declared
      failure with retry action (guardrail: no silent failure)
- [ ] edits, replies, reactions: create and render, declared in accessible text
- [ ] typing notifications out and in (in = polite announcement kind, default per
      verbosity)
- [ ] property test on outgoing body handling (multibyte safety)
- [ ] manual NVDA pass; run checks

### Phase 4, Task 1: E2EE surfaces

- [ ] encrypted rooms send/receive (SDK does the work; UI declares state honestly:
      unverified device warnings not click-through-able silently)
- [ ] SAS verification dialog: emoji by localized name plus decimal fallback,
      fully keyboard operable, tested with NVDA
- [ ] key backup and recovery-key flows, accessible (no memory-test auth; 3.3.8)
- [ ] cross-signing state surfaced in account settings
- [ ] manual NVDA pass; run checks

### Phase 4, Task 2: Media and files

- [ ] incoming images: alt text announced when present, "image, no description
      provided by sender" when absent; open-externally action with scheme
      allowlist via `opener`
- [ ] outgoing images/files: alt text prompt (skippable, never blocking), size
      confirmation, progress declared politely and boundedly
- [ ] no inline media rendering in v1 beyond thumbnails with accessible names
- [ ] tests for allowlist (file://, ms-settings: etc. refused); run checks

### Phase 4, Task 3: Notifications and sounds

- [ ] desktop notifications behind one trait; per-platform crate per the gap
      analysis; respect OS do-not-disturb
- [ ] audio cues via rodio: distinct per kind (message, mention, error, connect,
      disconnect), each with visual and announced equivalent, all optional
- [ ] test: cue distinctness (no two kinds share a sound), flood bound shared with
      announcement engine
- [ ] manual pass with sounds on and NVDA running; run checks

### Phase 5, Task 1: Settings

- [ ] settings dialog (GUI from the start, per Wixen Terminal lesson: an a11y app
      must be configurable without editing files): verbosity per event kind,
      per-room overrides, sounds, notifications, appearance (respects system
      contrast and text scaling), keybindings
- [ ] TOML persistence in `wixen-chat-config` with schema and drift-guard test
      (defaults in file match defaults in code)
- [ ] global mute toggle with a dedicated shortcut, announced
- [ ] manual NVDA pass; run checks

### Phase 5, Task 2: Accessibility CI and hardening

- [ ] Axe.Windows scan job in CI against the built app (headed Windows runner),
      blocking on new violations
- [ ] proptest/fuzz pass over parser-ish surfaces (HTML subset to text, config)
- [ ] dead-code-hunter pass; reachability audit (every implemented subsystem has a
      non-test caller; the Wixen Terminal lesson)
- [ ] docs/a11y-verification.md complete for NVDA; VoiceOver and Orca passes done
      or explicitly gated with dates

### Phase 6: Verify acceptance criteria

- [ ] the four questions re-read against the shipped feature set; anything that
      fails the frame is cut or gated, not shipped half-done
- [ ] all guardrails verified: reachability, screen reader confirmation, no stubs,
      bounded feedback, gated capability, few-things-excellent, upstream gaps named
- [ ] full test suite green on all three platforms
- [ ] beta tag decision

### Final: Documentation

- [ ] README: what it is, status, install, keyboard basics (writing-craft)
- [ ] docs/principles.md: the four questions as written here
- [ ] docs/accessibility.md: what is supported, what is not, how announcements are
      controlled
- [ ] move this plan to docs/plans/completed/

## Technical Details

- **Rust edition 2024**, MSRV 1.93 (matrix-sdk 0.18 requirement; workspace pins it).
- **Message flow inbound**: matrix-sdk-ui diff → mapping → core value → (a) update
  channel → call_after → widget update on UI thread; (b) AnnouncementRequest →
  policy engine → Announcer.
- **Announcement request**: `{ kind: MessageNew | Mention | Membership | Presence |
  Typing | Connection | Error | Verification, room: Option<RoomId>, text: String,
  source_priority: Polite | Assertive }`; policy may drop, coalesce, delay, or
  upgrade; never downgrades Assertive.
- **Store**: matrix-sdk sqlite store under the platform data dir (`directories`),
  passphrase in keyring.
- **Config**: TOML under platform config dir; no code execution in config (chat
  client threat model differs from the terminal; no Lua here without a new
  decision doc).

## Post-Completion

**Manual verification:**
- Full screen reader passes: NVDA and Narrator on Windows, VoiceOver on macOS,
  Orca on Linux; JAWS if licensable. Structure present is not experience good.
- A sustained real-use trial in busy rooms (the announcement bounds only prove
  themselves in anger).
- Security review of keyring usage, store encryption, and the URL allowlist.

**External work:**
- Packaging: MSI or MSIX, dmg, flatpak; code signing certificates.
- Server-side: document the homeserver sliding-sync requirement if the spike
  found degradation.
- NVDA add-on interop check with Terminal Access (no interference; they serve
  different apps).
- VoIP, local encrypted search, spell check, i18n translations: documented gaps,
  each needs its own decision doc before work starts.
