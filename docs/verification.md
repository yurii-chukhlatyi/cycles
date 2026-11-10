# Development delivery verification

This is a local development delivery, not a complete cross-platform release certification. The approved product and technical plan contains U1–U8. Implementation is present for all units; U4 live Google verification and U8 Windows execution/signing remain incomplete.

## Unit coverage

| Unit | Implementation and evidence |
|---|---|
| U1 | Rust record/decimal model, typed native IPC, React rendering, schema/policy versioning and local build. |
| U2 | Chronological weighted-average accounting; immutable position/cycle ownership; separate account/currency/cycle cash; fees, partial exits and grouped option settlements. Core and application fixtures cover accepted arithmetic and ownership examples. |
| U3 | Real literal-text XLSX round trips, journal integrity, idempotent replay, stale-parent conflicts, source fingerprint checks, atomic replacement, previous copies and pending recovery. Native backup/restore workflow. |
| U4 | Native OAuth PKCE/state/loopback, OS credential storage, drive.file scope, Sheets append protocol, staged large commands and reload verification. Mock HTTP/lost-response and callback tests. Live suite implemented but not run without owner credentials. |
| U5 | Native/consolidated equity and gains, dated quotes/FX, TWR/XIRR, contribution-neutral volatility, risk completeness and valuation coverage. Exact and missing-evidence regression fixtures. |
| U6 | Position-first entries, preview/save/recovery, cycles, cash, history, data sources and backups. Component tests plus Mac native partial-close/cycle/cash/backup workflow. |
| U7 | Tracker preview, stable IDs, raw dates and option price units, opening basis/cash reconciliation, explicit exclusions and baseline-gated performance. 1900/1904 workbook fixtures and a local real-tracker dry run. |
| U8 | Mac native workflow and app package; Mac/Windows CI and cross-platform backup comparison are configured. Windows runner/installer smoke and distribution signing are unperformed. |

## Review

Code review: completed through `ce-code-review`, receipt status `complete`, run `20261109-190110-fc85c3ab`. The initial verdict was “Not ready” with six findings. All six implementation issues were addressed; details are in `review-resolution.json`. The review ran sequentially in the main thread under the user’s AGENTS mapping, so it does not provide independent reviewer coverage. The test-only driver changes received a focused follow-up scan for build exclusion, isolated state, permissions and credentials.

## Migration and privacy

The real tracker was read locally. Its supported position IDs, transaction dates, option premium units and currency cash controls reconciled in a disposable dry run. That dry run used synthetic cash allocations solely to exercise the adapter; it is not the investor’s approved account/cycle allocation and is not included in the deliverable. Original tracker files remain intact. The app requires the investor’s explicit allocations and exclusion acknowledgement before saving migration.

Published source and examples use anonymized sample data. No real tracker, bank statement, OAuth client JSON, token, account identifier or local pending intent is included.

## Remaining external verification

- Run the configured native suite and packaged app on Windows, including the macOS backup parity job. There is no remote/Windows host attached to this local delivery.
- Connect an owner Google Desktop OAuth client, then explicitly run `CYCLES_RUN_GOOGLE_LIVE=1 pnpm test:google-live`. The suite creates a disposable sample sheet and leaves it for inspection/removal. The local guard exits “Not run” without opt-in. Mock transport tests are not live API evidence.
- Configure owner signing credentials before public distribution. The Mac development app has no Developer ID signature/notarization; Windows signing was not attempted.

## Final commands

- `cargo fmt --all --check`: passed.
- `cargo test --workspace --locked`: 33 passed; 1 explicitly ignored live Google test.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo clippy -p portfolio-cycle-app --all-targets --features e2e -- -D warnings`: passed.
- `pnpm install --frozen-lockfile`, `pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm build`: passed; 7 UI tests.
- `pnpm test:desktop` on macOS: passed its native local workflow; foreign-platform restore skipped without a Windows fixture. Verified a 5-share partial close leaves 75 shares, realized P/L 502.50 and combined P/L 2166, then balanced cycle transfer, permanent position ownership, winding down, invalid-file rejection and complete XLSX restore.
- Actual tracker dry run: 85 supported positions and 167 transactions; dates/IDs/option units and cash controls retained. Test allocations remain private intermediate data.
- Sample XLSX: 13 literal-input tabs, no formulas. Native cross-platform restore is skipped on this Mac when no foreign-platform fixture is supplied; that skip is not counted as a Windows pass.


## Mac development package

`pnpm tauri build --bundles app` passed and produced an optimized arm64 `Cycles.app`. The final package was launched through macOS, reopened its saved revision-2 XLSX, and displayed 75 shares, basis 7501.50, realized P/L 502.50, unrealized P/L 1663.50 and combined P/L 2166. Transaction dates and child-ledger rows were visible. The overview displayed net external contributions and allocated capital of 30000. Screenshots in the delivery show fictional sample data after this partial close.

The normal dependency tree excludes `tauri-plugin-wdio`. The production binary/frontend were checked for `plugin:wdio`, `wdio-webdriver`, `TAURI_WEBDRIVER_PORT` and `CYCLES_TEST_DATA_DIR`; none were present. Production config does not enable the global test bridge or `unsafe-eval`.

The executable has a linker ad hoc signature, with no Developer ID team or notarization. This is not a signed distribution release. A macOS app bundle was built and launched; Windows installers and live Google integration were not exercised.

All six review findings were handled inline; no batches were delegated. Finding 1 has its missing implementation in place while its Windows/live acceptance prerequisites remain pending. No required implementation finding was silently skipped. The original review receipt is preserved as `code-review.json`; it is the pre-fix review, not a second independent approval.


## Dark theme update

The interface now uses a Wealthsimple-inspired warm dark palette: charcoal surfaces, soft ivory text, muted gold controls/charts and mint/coral P/L colors. Shared CSS tokens cover the overview, position ledgers, editors, notices, native inputs and chart tooltips. The HTML and native window both select dark appearance.

Verification: the existing 7 UI tests, TypeScript/build and lint pass. The Mac arm64 release build passes and was launched, reopening the same verified workbook at revision 2 with equity 32166 and total position P/L 2166. Browser checks covered the overview, expanded position and entry form at 1280px and 780px desktop widths. A one-off contrast audit checked normal text and semantic text pairs at >=4.5:1; the lowest checked pair was muted text on raised surfaces at 5.55:1. No new behavior tests were added for the styling-only change; visual checks are the primary evidence. Rust accounting and persistence code were not modified.

Code review: skipped (mechanical diff) — color token substitutions, chart presentation, dark appearance configuration and documentation only. Manual diff inspection confirmed calculation expressions, events, data storage and permissions are unchanged. The earlier Windows/live Google verification limitations still apply; this update does not establish those external results.


## Semantic color update

The UI/UX designer was consulted in a separate sub-agent and approved the actual rebuilt native Positions screen with no material color or hierarchy issue. The palette uses mint/coral for gains/losses, blue/violet for call/put, cyan for open/active, amber for partial/winding-down/warnings, and slate for closed. Calls and puts are labeled in collapsed rows. Transaction actions have labeled glyph badges. Zero and unavailable results stay distinct. Outcome signs/arrows and explicit state/type labels support interpretation without color. Portfolio equity, contributions, and volatility stay neutral; the equity chart stays gold with a slate-blue capital line.

Verification for this change:

- 11 UI tests pass, including a mixed-row integration test for losing calls, profitable closed puts, stale/missing quotes, and zero/unavailable results. The new semantic component tests first failed before implementation, then passed. These are presentation tests, not new accounting evidence.
- TypeScript/Vite build, ESLint and the Mac arm64 release build pass.
- A one-off audit checked 28 semantic foreground/background pairs; minimum contrast 5.44:1. This is a palette check, not a complete accessibility certification.
- The final Mac app opened and verified a generated fictional 6-position XLSX, with profitable calls and puts, a losing closed call, a profitable closed short put, a partially closed stock and a losing open stock. The Rust engine calculated all example financial results. Position rows, expanded dated transaction badges, overview totals and returns were inspected in the actual native app.
- A 780px browser check confirmed the new legend wraps, P/L cards remain readable, and the positions table scrolls within its container without page overflow. It used the existing fictional read-only sample.
- Inline correctness, testing and maintainability review found no actionable code issue. The semantic helpers reuse existing formatting/sign functions; no accounting math, persistence, permissions or record contracts changed. Review ran in the main thread under the user's AGENTS mapping. It is not an independent code review or the default multi-agent finish workflow. Independent coverage here is the designer's visual review.
- Simplification pass found no worthwhile behavior-preserving code change. No test was removed or weakened.

The designer's optional polish notes were the existing persistent success banner and the dashed underline on Short; neither blocked approval. VoiceOver/keyboard accessibility, Windows execution and live Google Sheets were not tested by this change. The earlier distribution signing limitations still apply. All screenshots and the showcase workbook contain fictional data; real investor workbooks and statements were not modified.


## Positions currency toggle fix

The CAD/USD control updated the selected currency but the Positions list filtered only by search and open/closed status. The list now also filters on the instrument's native currency. The selected button exposes its pressed state, tooltips explain the native-currency behavior, and row counts/empty states identify the currency. This follows the existing separate-native-currency model; it does not convert USD positions to CAD.

Two new regression tests in the existing workflows suite failed before the fix because USD rows remained after clicking CAD. All 13 UI tests now pass, including switching back to USD and combining currency with search and status. TypeScript/Vite build, ESLint and the Mac arm64 app build pass. An isolated copy of the production frontend, populated with Rust-generated fictional records, was clicked through USD (6 rows), CAD (1 row) and back to USD (6 rows). Screenshots show the actual browser preview. Native interaction checks were interrupted while the app was being used; no native click-verification pass is claimed for this fix. The running old app must restart to load the rebuilt bundle.

Code review: lite inline correctness review completed with no actionable findings on the fix-only diff. No reviewer agents were dispatched. Simplification was skipped: the implementation is one currency predicate plus presentation labels, with no new helper or abstraction. Existing Rust financial calculations and persistence were untouched. No source data was modified. Windows/live Google verification and signing limitations remain unchanged.


## Dates before the first cycle

The Rust report returns no currency totals before the selected cycles have started. Overview previously gated its entire dashboard on a matching total, and its currency controls were generated from those missing totals. The dashboard now remains mounted, missing amounts use the existing Unavailable display, and a no-recorded-activity status explains the date/selection. Currency choices fall back to the selected accounts' configured currencies and retain the selected currency when valid. No financial zeroes or historical values are synthesized.

The regression test changes As of to 2020-01-01, checks all four headline cards and the major sections, switches currencies, checks Positions, and returns to a date with data while retaining CAD. It failed before the fix because Portfolio equity disappeared. All 14 UI tests pass. TypeScript/Vite, ESLint and the Mac arm64 build pass. A headless Rust report confirmed empty totals before cycle start. The production frontend was visually checked using a Rust-generated fictional future-cycle workspace; widgets stay visible and CAD/USD controls work without totals. This browser check is not native interaction evidence; the live app session was left alone and needs a restart to load the rebuilt bundle.

Lite inline correctness review found no actionable issue. A scoped simplification pass found no worthwhile change: nullable formatting reuses existing helpers and currency fallback adds no state, I/O or financial calculation. Independent code review, Windows and live Google checks were not run for this update. Existing signing limitations remain. Real workbooks and bank statements were not modified.


## Chronological date controls

The shared filter bar now places From before To. From replaces Period start and remains optional (empty means all available history); To replaces As of and remains the ending valuation date. Both inputs retain their original data bindings and date-change handlers. Tooltips explain the two meanings and the no-data notice now refers to the end date.

The existing date regression test uses the new To date accessible label; no new test was added for this reversible presentation change. All 14 UI tests, TypeScript/Vite build, ESLint and the Mac arm64 build pass. A production browser preview confirmed the visual and accessibility order From then To, with From blank and To set. Manual diff review verified the date bindings and computations were preserved. Dedicated code review and simplification were skipped for this mechanical reorder/copy edit. Native interaction, Windows and live Google were not repeated; existing signing limitations apply.


## Widget explanations

All seven requested Overview widgets now have question-mark help icons. Strategy performance, Exposure & data coverage, and Investment gain reconciliation also have short descriptions and help for their individual values. The four headline cards receive tooltips only. Help explains cumulative snapshots through To versus From-to-To performance, capital additions, fee treatment, exposure limits and missing evidence. The copy was checked against the existing Rust reporting, performance and attribution code; financial calculations and data storage are unchanged.

Help opens on hover, keyboard focus or click. Escape or clicking outside dismisses it. Each trigger has an accessible name and a description relationship to its tooltip. The portal keeps help clear of panel clipping, follows scrolling/resizing, stays within the viewport and closes when its trigger scrolls out of view.

Three new behavior tests failed before implementation, then passed with the existing fourteen tests. All 17 UI tests, ESLint, TypeScript/Vite build and the Mac arm64 application build pass. Actual browser checks covered wide and 780px windows, right-edge tooltip bounds, keyboard focus and Escape. Screenshots use fictional data. The user's running native app was left alone; restart it to load the rebuilt bundle. This is browser interaction evidence, not a new native interaction pass or VoiceOver certification.

The ce-code-review receipt is in widget-help-review.json. Correctness, testing, maintainability and frontend lifecycle checks ran sequentially in the main thread under the user's AGENTS mapping, with no actionable findings. No independent reviewer or peer ran; this is not the default multi-agent finish workflow. A simplification pass retained one shared tooltip component and explicit help copy bindings; no worthwhile further refactor was found, and no test was removed or weakened. Windows, live Google Sheets and distribution signing limitations remain as documented above.
