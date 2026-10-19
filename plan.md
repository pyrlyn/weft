# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T8 | in progress | P1 | 3 | 70% | Claude Code / claude-opus-5-5 |
| T14 | in progress | P2 | 5 | 75% | Claude Code / claude-opus-5-5 |
| T69 | todo | P2 | 5 | 0% | |
| T106 | in progress | P2 | 5 | 95% | Claude Code / claude-opus-5-5 |
| T106.7 | in progress | P2 | 4 | 95% | Claude Code / claude-opus-5-5 |
| T105 | in progress | P2 | 5 | 30% | Claude Code / claude-opus-5-5 |
| T903 | todo | P2 | 2 | 0% | |

### T8. Evaluation

T20–T25 have landed, so the final runs are the ones on the Rust core and the stricter benchmark. The harness steps below are in the tree. The full edit and read runs are not: this environment has no `ANTHROPIC_API_KEY` and no LM Studio server, and the runs already in `test.md` are the earlier one-sample measurements.

Full benchmark run on two or three models against the baselines, and a report with a continue/stop recommendation. Done when first-try validity is at least 95%, validity after one repair cycle is at least 99%, and raw results are in the repository.

Execution plan:

1. Repair cycle in `bench/src/run-tasks.ts`: when an edit reply is invalid (unparseable or format errors), send one follow-up prompt with the previous reply and the validator's diagnostics, and record first-try and after-repair validity and success separately. Same rule for every format, so the baselines get the same help. Cover it with a mocked-provider test in `bench/test/checkers.test.ts`.
2. Summaries and the rendered table show first-try valid, valid after repair, first-try success, success after repair, and mean output tokens.
3. Models: Claude Sonnet 5.5, Claude Opus 5.5, Claude Haiku 4.5 (`DEFAULT_MODELS`).
4. Raw results go into the repository: stop ignoring `bench/results/`.
5. Trial run (one screen, all formats, one model) to check cost and the harness; the creator reviews it before the full run.
6. Full edit and read runs on the three models; `run.ts tokens` again so the report has Anthropic token counts.
7. `bench/EVALUATION.md`: results against the done criteria (Weft first-try validity at least 95%, after one repair at least 99%), comparison with the baselines, failure analysis, continue/stop recommendation.
8. Verify with `pnpm run ci`.

Progress: steps 1–5 are done (repair cycle, summaries, `DEFAULT_MODELS`, results committed under `bench/results/`, trial row in `test.md`). `bench/EVALUATION.md` records those runs against the numeric bar and does not close the task. Remaining: step 6, the three-sample edit and read runs on the current harness, then a history row and a revision of the report. Step 8 waits on that.

### T14. Figma round trip and plugin

Convert Weft to Figma and back without loss, and ship a Figma plugin. Approved scope:

- **Library:** generate a Weft component library in Figma from the catalog and the design tokens (one component set per catalog kind, variants for its enum props, Figma variables for the tokens). Screens are built from instances of this library.
- **Weft to Figma:** every node becomes an auto-layout frame or a library instance, and its Weft source (kind, props, bindings, data paths, ids) is stored in the node's plugin data.
- **Figma to Weft, lossless:** a screen that was never edited comes back byte-identical after `weft fmt`. A designer's edits come back too: text, order, added or removed library instances, variants, and visual edits (spacing, colors, sizes, radii). A visual edit maps to a token when it matches one and otherwise to a style override, which needs a format extension (`AGENT-SPEC.md`, the parsers in TS and Rust, the catalog). Its design goes to the creator before it is built.
- **Foreign layers:** layers that did not come from Weft (vectors, images, free frames) convert lossily, with a loss table, as before: Figma layers carry no semantics.
- **Where it runs:**
  - a Figma plugin: open or paste a `.weft` file to build frames; select a frame to export `.weft`.
  - CLI `weft figma pull`: reads through the REST API (`GET /v1/files/:key/nodes`). The REST API is read-only, so building frames stays in the plugin.
  - MCP / Claude Code tools (with T30), working through the Figma MCP server.

Done when every corpus screen survives Weft to Figma to Weft byte-identical, a scripted set of designer edits comes back with the expected Weft diff, and the plugin, CLI command and MCP tools pass their tests in `moon ci`.

Execution plan, stage 1 (library, both conversions, plugin; the CLI command, the MCP tools and style overrides are later stages):

1. `packages/figma` (`@weft/figma`), pure TypeScript against a narrow interface of the Plugin API subset it uses. A type test checks that the official `@figma/plugin-typings` types satisfy that interface, so the real `figma` object and the in-memory fake (`test/fake-figma.ts`) are interchangeable.
2. Library: one page with a component set per catalog kind (variants for its enum props and `state`, with an `(unset)` value where the prop has no default) and one variable collection holding the design tokens (dimensions as FLOAT in px, colors as COLOR). Kinds and tokens are recorded in plugin data, so a second run finds and reuses them.
3. Weft to Figma: kinds whose content is `none` or `text` become library instances, the rest auto-layout frames (`stack` direction, gap bound to its token variable, align, wrap; `grid` columns); `<each>` and named slots become marked frames; text children become text layers. Each node keeps its Weft source (kind, id, props, events) in plugin data, and the root keeps the document version.
4. Figma to Weft: a node is rebuilt from its stored source, and the Figma state is compared with the state that source would build. What is equal keeps the source as written (so an unedited screen is byte-identical); what differs is the designer's edit: text, label, child order, removed nodes, variant props, hidden, stack direction/align/wrap, and a gap that matches a token (bound variable or equal value). Copied nodes get fresh ids. New library instances become new nodes with generated ids and stand-ins for required props. Visual edits with no Weft prop or no matching token are reported as losses.
5. Foreign layers (text, free or auto-layout frames, groups, images, vectors) convert lossily with a loss table in the shape of `@weft/from-aria`; reuse its id, literal and limit helpers.
6. `plugins/figma`: `manifest.json`, a thin `code.ts` over `@weft/figma` (build from pasted or opened `.weft`, export the selected frame), `ui.html`; bundled into one file with Vite, with a test that the bundle builds.
7. Tests (Vitest): every corpus screen round-trips byte-identical through the fake; a scripted set of designer edits yields the expected Weft; foreign layers yield the expected losses; library generation is idempotent.
8. `packages/figma/README.md` with the Plugin API facts the code relies on, each with its official URL and the date checked. `docs/figma-style-overrides-design.md`: the style-override proposal for the creator (not built).
9. Wire into pnpm, moon, `tsconfig.json`, `toolchain.md`; verify with `mise exec -- moon run :test root:typecheck root:lint root:rust-test root:rust-lint`.

Progress (stage 1 done, on branch `t14-figma`):

- Steps 1–9 are done. `@weft/figma` builds the library, both conversions, foreign layers and the plugin message handler. `plugins/figma` holds the manifest, a thin `code.ts`, `ui.html` and the Vite bundle. The bundle is tested by running it in a bare VM over the fake.
- Tests:
  - all 12 corpus screens round-trip byte-identical with no losses;
  - 19 scripted designer edits match the expected Weft patches and loss kinds;
  - building the library twice reuses it.
- `from-aria` now exports `freshId`, `literal`, `fillRequired` and the limit helpers, which both importers share.
- After T22 (WebAssembly core), the plugin is split. Figma's main thread has no WebAssembly, so it only builds and reads layers (`code.js`, with the core's loader stubbed). The UI iframe parses and serializes with the core inlined in `ui.html`. A harness runs both halves, the main thread without WebAssembly. The main thread interprets no values: the UI formats `text` and `label` for the build, and `finishRead` reads typed text with the core's `readValue` (a new `weft-wasm` export), so the value rules have one implementation.
- `docs/figma-style-overrides-design.md` waits for the creator's approval.
- Remaining:
  - style overrides: after approval, the format extension in TS and Rust, then the Figma mapping for colors, radii and padding (only `gap` maps to a token today);
  - a pull from a real Figma file, to confirm the REST facts marked unverified in `packages/figma/README.md` (rotation in radians, number precision, the shape of `sharedPluginData`);
  - MCP / Claude Code tools (with T30): `use_figma` reads and writes the shared plugin data (T14.4); the Weft tools on top of it are still to be designed;
  - a check of the plugin in the real Figma app, including the inline module script and WebAssembly in its UI iframe;
  - the open questions in the report: single components for kinds without variants, `state` as a variant axis, and the manifest id (plugin data on duplicate and detach is settled by T14.4).

Execution plan, stage 2 (this round; style overrides stay out until the creator approves them):

1. T14.1: the pull over the REST API — done (`done.md`).
2. MCP / Claude Code tools through the Figma MCP server. The Figma MCP server's `use_figma` tool runs Plugin API code, but its own description says `setPluginData` is not supported there, and plugin data is readable only by the plugin that wrote it. The creator chose to move the Weft source to shared plugin data, which any plugin and the REST API (`plugin_data=shared`) can read: T14.2, done (`done.md`).
3. T14.3: the creator dropped files that keep the Weft source in private plugin data; only shared plugin data is supported — done (`done.md`).
4. T14.4: `use_figma` reads and writes shared plugin data, checked in a real test file; the fake follows real duplicate, detach and instance inheritance — done (`done.md`).


### T69. Figma-like desktop design editor

A desktop design editor in the spirit of Figma for Weft documents, on macOS and Windows. Weft is the document format: an editor document is a set of Weft screens (`.weft`, canonical markup and JSON, SPEC §2–§3) with the project's catalog and tokens (SPEC §10), so whatever the editor saves validates, formats and converts with the existing tools, and agents edit the same files through the CLI and MCP server.

**Prerequisite: the Slint bindings.** The macOS (Swift) and Windows (WinUI 3) integration MUST use the Slint bindings of T68: the project `~/GitHub/listepo/apps/slint-bindings`, repository `listepo/slint-bindings` (https://github.com/listepo/slint-bindings), with its crates `slint-bindings-core` (custom `slint::platform`, software renderer, window adapter, input) and `slint-bindings-ffi` (C ABI), its `swift/` package (a Slint surface in SwiftUI through `NSViewRepresentable`) and its `windows/` package (the same surface in WinUI 3). No separate or ad-hoc embedding lives in Weft. A gap found there (input, IME, focus, DPI, accessibility, rendering, lifecycle) is fixed in slint-bindings and consumed from it, never worked around here. T68 is done (`done.md`). Weft → Slint is done: T67 and T67.1–T67.5 (`done.md`) cover the whole catalog and the read-back of edited Slint.

**Architecture split.**

| Part | Built with | What it holds |
| --- | --- | --- |
| Editing surface (shared) | Slint, embedded through slint-bindings, one code base for both platforms | The canvas (pan, zoom, rulers, grid, snapping), the rendered screens and frames (each Weft screen rendered through the T67 converter), selection, hover and resize handles, the layers tree of the edited design, the property inspector of the selected element (props, bindings, tokens, states, events, slots), the component and catalog panel (insert a kind, frames and component instances), and any other editing widget that must behave the same on both platforms |
| App shell (native) | SwiftUI and AppKit on macOS, WinUI 3 on Windows | Windows and tabs, menus and the menu bar, toolbars, file open and save dialogs, settings, the document browser and recent files, sidebars and panels outside the editing surface, notifications, the platform's undo menu items and keyboard shortcuts wired to the shared core |
| Document core (shared) | Rust, the Weft crates | Loading and saving `.weft` files and projects, validation (`weft-core`), every edit as a Weft patch (SPEC §7) so undo and redo are patch inverses and agents' patches apply the same way, the catalog and tokens (`weft-catalog`), and the Weft ↔ Slint conversion (`weft-slint`) that renders the screens and reads edits back |

The native shell and the shared surface talk only through the slint-bindings C ABI and a small editor API on top of it (open, save, select, apply patch, events back to the shell); neither side reaches into the other's toolkit.

**Milestones** (each becomes a T69.x task with its own plan when claimed, within the task size rule):

1. Skeleton: done (T69.1, `done.md`). A native window on each platform hosts one Slint surface through slint-bindings, shows one corpus screen rendered by `weft-slint`, and opens and saves through the native file dialogs.
2. Canvas: several screens as frames, pan and zoom, rulers, selection and hover from the Weft element tree, keyboard navigation.
3. Editing: move, reorder, insert and delete elements, resize within the layout rules, the layers tree and the property inspector writing Weft patches; undo and redo; validation diagnostics shown on the canvas and in the inspector.
4. Components and tokens: the catalog panel, catalog-extension kinds as components, design tokens from the project (spacing, colours, typography) in the inspector, the light and dark appearances.
5. Round trip with the outside: the saved files stay canonical (`weft fmt` leaves them unchanged), the existing exporters (SwiftUI, web, Slint, A2UI) run from the native menus, and files edited by agents or by hand reload without losing the selection where ids still exist.
6. Packaging: signed builds for macOS (arm64 only) and Windows x86_64, with the toolchain rows in `toolchain.md`.

**Acceptance criteria.**

- The same Slint editing surface code runs inside the SwiftUI app on macOS and inside the WinUI 3 app on Windows through slint-bindings, with no editor-specific embedding code in either shell.
- App chrome (windows, menus, toolbars, dialogs, settings, document browser) is native on each platform and contains no Slint.
- Every corpus screen opens, renders, and saves back byte-identical when nothing was edited; a scripted set of edits (move, insert, delete, change a prop, bind a prop, apply a token) produces the expected Weft diff, and the result passes strict validation.
- Undo and redo restore the exact previous document; edits are recorded as Weft patches.
- Tests: unit tests of the document core and the editor API, integration tests of the scripted edits over the corpus, and a check that every rendered screen's Slint compiles; the native shells have UI smoke tests on their platforms.

**Out of scope.** Multiplayer and real-time collaboration, cloud storage, accounts and sharing, comments and review threads, prototyping and interaction flows, vector drawing tools and boolean shape operations, image editing, plugins for the editor, Figma file import (T14 covers the Figma round trip), a web version (later, through Slint's WebAssembly build), iOS and Android, Intel Macs, and Linux.

Milestone 1 is done (T69.1, `done.md`). The next milestone is the canvas.

### T106. Weft plugin for cox

The creator asked for a plugin for cox (`~/GitHub/listepo/apps/cox`, https://github.com/pyrlyn/cox), written in Rust, and allowed extending cox where its plugin API falls short. A cox plugin is a WebAssembly module (extism guest, `wasm32-unknown-unknown`) with a `plugin.toml` manifest, written with cox's Rust SDK (`cox-plugin-sdk`, `plugins/sdk` in cox); its `init` returns the model-callable tools (`ToolSpec`s), served through `cox_tool_call`. The plugin gives the cox agent the Weft tools the MCP server gives other hosts (same names, inputs and outputs) by calling the Rust crates directly: no Node, and no second copy of the behaviour. Later parts give the cox desktop app a Design tab, like Claude Design: the agent's screens rendered as real UI that a person can look at and select from.

Done when T106.1–T106.8 are done. Sub-tasks, in order (each within the task size rule):

- T106.1: the MCP tool logic in Rust (`weft-binding`).
- T106.2: the TypeScript MCP server on that Rust logic.
- T106.3: `plugins/cox`, the cox plugin with the tools.
- T106.4: the accessibility snapshot of `weft_render` in Rust, so cox gets `weft_render` too.
- T106.5: skills in cox: a cox extension for plugin-carried skills, and the Weft skills in the plugin.
- T106.6: Design tab, design: these cards and a cox design proposal (`docs/design/` in cox), before code.
- T106.7: Design tab, cox side: the plugin-API extension and the desktop host (cox tasks).
- T106.8: Design tab, Weft side: the plugin renders the screens, keeps the tab live and passes the selection to the agent.

What exists (checked on origin/main): the MCP server is `packages/mcp` (nine tools: `weft_primer`, `weft_capabilities`, `weft_catalog`, `weft_schema`, `weft_validate`, `weft_format`, `weft_patch`, `weft_render`, `weft_context`), bundled into the Claude Code, Cursor and Open Design plugins by `plugins/shared`. Its checks already run on the Rust core through `@weft/core` and `@weft/catalog`, but the tool layer itself (primer text, limits, diagnostics truncation, project scoping, the capabilities shape, the catalog index, context filtering, the tool descriptions) is TypeScript, and `weft_render`'s accessibility tree and page come from `@weft/render-react` (TypeScript only). `crates/weft-binding` holds the JSON-text functions both engines call; `crates/weft-web` has the static HTML/CSS generator (`to_html`). cox has no way for a plugin to carry skills (`plugin.toml` has `[[agents]]` and `[[mcp]]`, no skills) and its desktop plugin API is text widgets only (PL§15.4 lists plugin web views as "never", marked open in A134), so T106.5 and T106.7 extend cox.

### T106.7. Design tab: cox side

The cox tasks the T106.6 proposal defines, as built: T33.47.1–T33.47.10 in pyrlyn/cox (`docs/design/plugin-canvas.md`, A156, branch `t33.47-plugin-canvas`); cox's A157 split T33.47.8 into T33.47.8.1–T33.47.8.10, one layer each, and added T33.47.10, a fix. They cover the plugin-API types, manifest key and grant line, the host calls and SDK arms, the `cox-app` state, wire and FFI, the isolated `CanvasPage`, the pane, its shortcut and palette rows, and the user docs. Done when their checks pass in cox and the branch is pushed. The Weft plugin's `cox-plugin-sdk` pin moves to that branch in T106.8.

Status: every cox card is done and pushed (branch head `fd35f0d0`) but T33.47.8's last checks: by hand, ⌘⇧D and the palette row toggle the pane, two frames show, and a click adds a chip that a second click replaces; and the falsifier 3 measurement in Instruments. Both need a person at the app, so they are with the creator.

### T105. Weft to WinUI 3 and back

Convert a Weft screen into a WinUI 3 page for a Windows App SDK app, and read WinUI XAML back into Weft, as T67 does for Slint and T34 for SwiftUI. The design is `docs/winui-design.md`; the external facts, with sources, are `research.md` §23.

**Decisions (creator).** A new target modelled on `crates/weft-slint` and `crates/weft-swiftui`. The output is a XAML `Page` that binds with `{x:Bind}` to a generated C# view model: data paths are its properties, the actions of `on-<event>` are its commands. The importer reads the XAML only; the C# is derived. Checked locally by golden files, XML checks and round trips, and in CI by a Windows job that `dotnet build`s a WinUI project holding every generated corpus screen; screenshots may follow as a separate task. The creator approves the design before any code.

**Execution plan (this round, design only).** Write `docs/winui-design.md` (mapping of every core kind and prop, tokens, layout vocabulary, bindings, events, slots, light and dark, losses, round trip, XML parser, CI job, open questions), `research.md` §23, and the sub-tasks below. The creator approved the design and accepted the recommendation of every open question in `docs/winui-design.md`, including a `roxmltree` row in the workspace `rust.md` when T105.8 adds the crate; T105.1 may start.

**Done when** every sub-task is done: every corpus screen and catalog example generates, its golden files build in the Windows job, `import_winui(generate(x)) == x` for each, `read_winui` reads the generated XAML without its source comment with only the losses SPEC §9 lists, and SPEC §9, §10.6, AGENT-SPEC and the crate README describe the target.


### T903. macOS studio UI test: Save As writes nothing locally

Seen by both loop workers: locally, the macOS `studio:test` XCUITest fails with "Save As wrote nothing", while the same test passes in CI. Find the cause (likely focus or timing around the save panel, as T902 found for Windows) and fix it, starting from a reproduction. Done when the test passes locally several runs in a row and stays green in CI. Needs a Mac with Xcode, so it cannot run in a Linux cloud session.

