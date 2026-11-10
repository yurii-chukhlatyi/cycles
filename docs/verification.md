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
