# Cycles

A macOS and Windows portfolio tracker with a Rust accounting engine and a Tauri/React interface. Google Sheets or a local XLSX is the durable source. The app performs every ledger and performance calculation; spreadsheet formulas are neither generated nor required.

## Run

Install Rust stable and Node.js 22, then enable pnpm 11.1.2. macOS needs Xcode Command Line Tools. Windows needs the Visual Studio C++ build tools and WebView2.

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

Choose **Data & source** to create an empty workspace, load the clearly labelled sample, or open an app-controlled XLSX. Save an in-memory workspace as XLSX or create its Google Sheets source before closing the app.

## Portfolio workflow

1. Create accounts and enable their currencies.
2. Create a cycle with a start date and strategy.
3. Record initial capital or allocate an external addition to that cycle. Transfer only uncommitted cash from another cycle.
4. Open a position, then expand its row to add or correct transactions. Separate IDs represent separate accounting lots; entries within one ID use running weighted-average basis.
5. Add dated quotes, option deltas, underlying prices, and exchange rates in **History**. CSV quote imports use `instrument_id,date,price,delta,underlying_price,adjusted,source`; the last four columns are optional.
6. Preview the complete command, including quantities, basis, P/L and both cash endpoints. Save reads the spreadsheet back before showing success.
7. Finish a strategy by winding down its cycle. Existing holdings continue updating and settle in their original cycle. A cycle can close only when its positions and lifecycle obligations are settled. Residual cash may remain or be transferred.

**Total position P/L** is retained-history realized plus current unrealized P/L. A date range instead shows the realized change plus the change in unrealized value. Investment gain is separately measured from the supported baseline after capital flows, with income, expenses and reconciliation differences visible. The position-P/L card is not necessarily lifetime account profit.

## Google Sheets

Create a Google Cloud **Desktop app** OAuth client and enable the Google Sheets API. Add your account as a consent-screen test user when applicable. Download the installed-client JSON, then select **Connect Google** in the app. Sign-in opens the system browser and returns to a random loopback port with PKCE and state validation.

The app requests `https://www.googleapis.com/auth/drive.file`. Create a source from the workspace, or reconnect an app-created sheet by its ID/URL under the same Google account and OAuth client. Arbitrary existing spreadsheets are imported through local XLSX. Access and refresh credentials stay in macOS Keychain or Windows Credential Manager; they never enter the React renderer, spreadsheet, backup, or logs.

A client JSON is not included in the source or release. Live OAuth and Google persistence must be verified with your client and account. Do not commit client files or tokens.

## Storage and recovery

The active spreadsheet contains Meta, Settings, Accounts, Cycles, Instruments, Positions, Transactions, CashMovements, Reservations, MarketData, FxRates, ValuationInputs, and Commits.

Each data sheet contains immutable revisions with `record_json` as the exact canonical input, followed by inspection columns. The JSON permits optional instrument and lifecycle details without lossy numeric spreadsheet coercion. All cells are literal text. Edit records through the app: direct edits break integrity validation and block writes rather than silently changing the ledger. Formatting is disposable.

A command's manifest references all its record hashes and its expected parent. Replay accepts the first valid successor, rejects a stale competing command without a domain effect, and applies a repeated command ID/payload once. Large Google commands stage raw rows before publishing their manifest. The commit log is the authority, not the last occupied data row. A conflicting command remains visible in source replay diagnostics.

Local saves hold a cooperative sibling lock, verify the source fingerprint, write and flush a temporary workbook, reopen it to verify replay, recheck the original fingerprint, preserve `.FILENAME.previous.xlsx`, and atomically replace the primary file. An external editor that ignores the lock can still race a file replacement; this is not database isolation. Do not edit or sync the same local workbook simultaneously from multiple applications. Google Sheets is the preferred source across devices.

Pending write intents are kept under the platform application-data directory `com.portfolio.cycles/pending`. They contain unsaved raw records, not credentials. **Retry** checks the same command ID before writing. **Review / rebase** shows current records alongside the pending replacement and creates a fresh validated preview against the source's current head. A verified replacement clears the older intent. **Discard intent** removes only the local recovery request; it cannot undo a command already accepted by the source.

Export creates a complete, independently reopenable XLSX; it does not silently switch the primary source. Export refuses to overwrite an existing path. **Save workspace as XLSX** explicitly selects a new local primary file. Startup remembers the last selected source. No database or background price/sync service is used.

## Import the existing tracker

Create or open a destination workspace, select the original tracker and its **report as-of date**, and review the preview. Assign every supported position to its permanent cycle. Enter report-date opening cash for every account/cycle/currency bucket, including zero; allocations must reconcile to report controls. The app does not guess historical deposits or account-level cash ownership from aggregate controls.

The importer reads raw IDs, Excel/ISO dates, quantity, unit prices, fees and basis adjustments. It supports the 1900 and 1904 Excel date epochs. Option premiums in the supplied tracker are already per underlying unit and are not divided by 100 again. Opening holding snapshots establish basis without generating purchase cash. Original source fingerprints and row provenance remain attached to the records.

Existing assignment stock legs are retained and linked when the matching source event is unambiguous. An assignment without sufficient detail requires an explicit current-holdings/cash reconciliation. Unsupported commodities require an explicit exclusion; the original workbook remains intact. Bank and credit-card statement tabs are not imported.

Performance begins at the reconciled baseline. Historical prices, returns and funding are not manufactured. Your actual cash allocations and any exclusions must be reviewed by you before migration is saved.

## Calculation policy

- Stored amounts use decimal strings, checked decimal arithmetic, and at most 12 input decimal places. Final closes consume remaining basis exactly.
- Opening fees increase long basis or reduce short credit. Closing fees reduce realized proceeds. Trade cash is derived once from its trade, never duplicated as a deposit.
- Options use whole contracts and explicit multipliers. Exercise/assignment previews the option event and underlying delivery as one group at the strike. Cash-settled options do not generate shares.
- Cash-secured short puts reserve gross strike obligations. Covered calls uniquely bind eligible shares in the same cycle/account. Other short obligations require explicit reserves; unavailable commitment data makes transfer availability unknown.
- Quotes and FX use only evidence dated on or before the requested date. Historical return observations require dated quotes or recorded valuations. Missing observations stay missing; stale marks are labelled.
- Native currencies remain separate. An optional reporting currency adds consolidated equity, growth, TWR/XIRR, volatility and a profit/currency bridge when dated direct/inverse FX evidence supports them. Equity uses valuation-date FX; contributions and realized P/L use event-date FX. Native reconciliation differences are converted at ending FX before the currency translation/conversion spread residual is identified. Missing flow-date FX withholds return and gain even if current equity is known. Historical foreign-currency statistics require current dated FX evidence; the current equity mark can still show the last known rate with a coverage warning.
- Exact TWR uses no-flow intervals or reconciled, explicitly measured pre/post-flow valuation inputs in native currency. The consolidated view uses no-flow exact returns or the labelled daily convention. Otherwise complete daily observations use an explicitly estimated end-of-day-flow convention. XIRR is annualized, Actual/365, with a finite search and ambiguous/no-root status. Volatility uses sample standard deviation, 252-day annualization, a default 60 valid trading observations and a minimum of 20. Missing trading dates are not bridged. Adjusted asset quotes use a separate consistent series; unadjusted results are labelled price-return volatility.
- Stops, delta exposure, maximum loss, allocation and cash reservations are separate measures. Missing inputs never become a complete aggregate zero. A risk estimate does not represent a broker margin calculation.

## Check and package

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm lint
pnpm test
pnpm build
pnpm test:desktop
pnpm tauri build
```

`pnpm test:desktop` builds a separate test frontend with optional Rust WebDriver plugins and an isolated temporary application-data directory. It runs native WebdriverIO partial-close, cycle ownership, cash transfer, winding-down and complete backup/restore checks. Evidence goes to `test-results/native/`. Normal builds exclude the driver plugins, test permissions and frontend bridge. Never distribute the `e2e` binary.

`pnpm test:google-live` is an explicit owner-run check. First connect Google in the normal app, then run with `CYCLES_RUN_GOOGLE_LIVE=1`. It reads the authorized client and tokens only from the OS credential store, creates a disposable sample sheet, checks grouped settlement/transfers, concurrent-successor selection, idempotent retry, and XLSX parity. It prints the disposable sheet ID and leaves that sheet in Drive for inspection and removal. Without opt-in it exits with an explicit “Not run” result; mocked transport tests do not establish live Google behavior.

The GitHub Actions workflow runs the checks, native desktop suite and macOS/Windows installer builds, then restores the actual macOS-made backup on Windows and compares raw inputs and calculated reports. It has not run until the repository is pushed to a configured remote. A macOS build does not establish native Windows behavior. Public distribution additionally requires the owner's signing/notarization credentials; local development bundles are not Developer ID signed or notarized.

The headless `cycles-tool` uses the same engine and adapters:

```sh
cargo run -p portfolio-storage --bin cycles-tool -- sample sample.xlsx
cargo run -p portfolio-storage --bin cycles-tool -- inspect sample.xlsx
cargo run -p portfolio-storage --bin cycles-tool -- report sample.xlsx 2026-11-09
cargo run -p portfolio-storage --bin cycles-tool -- import-preview legacy.xlsx 2026-11-08 preview.json
```

`import` additionally requires a reviewed choices JSON containing `position_cycles`, `cycles`, `cash` allocations, and explicit reconciliation/exclusion acknowledgements. Run the tool without arguments for its command syntax.

The browser preview uses an anonymized Rust-generated sample projection and is read-only. It does not calculate or persist financial data in JavaScript.
