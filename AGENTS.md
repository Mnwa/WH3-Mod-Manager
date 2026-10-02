# Agent development rules

Adapted from `/Users/mnwa/RustroverProjects/cr-chat-desktop/AGENTS.md` for this
workspace. Chat-specific API contracts and medical-data rules do not apply here.

## Project stack

- Rust edition 2024; `crates/core` contains domain/file/persistence logic and
  `crates/desktop` contains the native application.
- Use the latest stable Rust. Do not pin a compiler version or add a `rust-version`
  requirement. Keep `Cargo.lock` committed for reproducible dependency resolution.
- Use GPUI through the `gpui-kit` facade: `gpui_kit::*` for GPUI and
  `gpui_kit::component` for controls. Reuse kit components before creating new controls.
- Use GPUI style methods and the shared palette in `crates/desktop/src/theme.rs`.
- Use `gh` for GitHub operations.

## Core rules

- Keep code readable, reusable and split into focused components. Compose screens
  from smaller views instead of mixing rendering, I/O and domain logic.
- Keep `main.rs` and `lib.rs` thin. Module facades declare/re-export their public API;
  a slice facade may also declare its shared state, but behavior belongs in focused
  sibling files. Prefer `pub(super)` for implementation details.
- Keep files near or below 300 lines where practical. Split a large `impl` by
  responsibility; do not create abstractions solely to satisfy a line count.
- All documentation, Rustdoc, explanatory comments and PR descriptions are English.
  Comments explain why a decision exists rather than restating code.
- Application-owned UI text must have English and Russian translations. Use
  `Language::text`, `ui_text!` or bilingual `Message` values. Never translate mod
  titles, paths, user preset names or imported user content.
- Preserve the original Shazbot copyright notice and the 2026 Mikhail Panfilov
  (Mnwa) notice. Workspace package authorship is inherited by both crates.
- Record functionality changes and remaining differences in `docs/MIGRATION.md`.
  Do not describe a preserved-but-unapplied feature as implemented.

## Architecture and dependency direction

Apply the reference project's Feature-Sliced Design principles to the existing
workspace rather than creating empty layers:

- `desktop/app` owns application startup, windows and root composition.
- `desktop/manager` owns the mod-management feature and its UI state. Keep rendering,
  actions, jobs, presets, metadata, persistence and language handling focused.
- `core` owns catalog, pack, preset, metadata, Steam, storage and launch behavior.
- `desktop -> core`; core never depends on GPUI, windows or desktop state.
- New independent screens/features get their own slice and public facade. Import
  other slices through that facade, not private implementation paths.
- Extract shared visual patterns into theme/UI helpers only when actually reused.
  Put state close to its owner; pure helpers and I/O belong outside render methods.

## GPUI and performance

- Stateful views use `Entity<T>` and `Render`. Stateless components use `RenderOnce`
  or plain functions. Add entities only when they own state, tasks or subscriptions.
- Children communicate through callbacks or events. They never reach into parents.
  Use model methods to change shared state and scope notifications to affected views.
- Never block the GUI thread with filesystem access, pack parsing, serialization,
  conflict analysis or large searches. Use `cx.background_spawn`.
- Own `Task`s and `Subscription`s in the view. Detach only deliberate fire-and-forget
  work. Use cancellation and generation checks so stale results cannot replace new state.
- Keep mod, preset and report lists virtualized. Never scan, sort, clone or lay out
  the whole library during render. Borrow data or share immutable snapshots with `Arc`.
- Normalize search data once, debounce typing and avoid per-keystroke catalog copies.
- Do not add SIMD, unsafe code or special allocators without a measured bottleneck.
  Record corpus, environment, build mode, raw samples and limitations for performance claims.
- Do not call `.hover()` twice on an element; GPUI can panic. Compose hover styling once.
- Add `.cursor_pointer()` to clickable kit buttons only while enabled, using `.when`.
- Preserve keyboard access, labels and tooltips. Icon-only controls need accessible
  names. Test both locales after layout changes.
- Use Lucide through `gpui_kit::assets::IconName` and kit assets. Do not copy SVG files
  into the repository. Keep icon size/alignment consistent with surrounding controls.

## Styling

- Use colors from `theme.rs`; no scattered hexadecimal colors in views.
- Preserve a compact, readable mod table with consistent spacing and typography.
- Reuse kit Button, Input, Checkbox and related controls. Do not replace real actions
  with manually clickable divs or introduce decorative controls without behavior.
- Separate table sorting from the actual game load order.
- Check visual screenshots for truncation, alignment, contrast and both language lengths.

## Rust, files and persistence

- Use typed errors; never unwrap I/O, network or user data. Keep platform-specific
  code behind `cfg` and ensure other targets compile without warnings.
- Borrow on hot paths; avoid copying entire catalogs or results for a row update.
- Keep original-manager imports read-only. Export must not overwrite the source config.
  Do not silently delete, rename or move game files to implement an option.
- Keep binary record schemas separate from domain structs. WHM1 changes require an
  explicit format version and migration; preserve the frozen compatibility fixture.
- Validate binary data before accessing it, retain atomic replacement and backup
  behavior, and never overwrite a damaged library with an empty fallback.
- Language preferences use the separate WHP1 record; do not silently alter WHM1 to
  store UI preferences. Demo mode must not write user settings.
- OS error details and third-party messages may retain their original language;
  application-owned explanations must be bilingual. CLI defaults to English.

## Required validation

After changes, run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Fix formatting differences, Clippy warnings and failing tests before completion.
The workspace test includes real headless Metal rendering on macOS. Inspect affected
screenshots in `target/visual`, including English and Russian variants. Other targets
explicitly skip that renderer test; a skip is not a Windows GUI runtime test.

For search/list/performance changes also run:

```sh
cargo bench -p wh3-core --bench catalog --locked
cargo test --release -p wh3-mod-manager --test visual --locked
```

Use meaningful regression tests for behavior, corruption handling and compatibility.
Avoid tests that merely mirror reversible cosmetic changes. GitHub Actions must pass
Linux core checks, macOS UI checks and Windows workspace checks. Keep quality checks
in `ci.yml` (pushes/PRs targeting `main`) and Windows release packaging/publication
in `release.yml` (an existing published GitHub Release).
Confirm that the executable artifact belongs to the final branch commit. Distinguish
successful Windows compilation from an actual game-launch/runtime verification.
