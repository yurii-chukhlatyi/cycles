import { useCallback, useEffect, useState } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  ArrowDownUp,
  ArrowUpRight,
  ChevronDown,
  ChevronRight,
  Clock3,
  Database,
  Download,
  FileSpreadsheet,
  Layers3,
  LayoutDashboard,
  Plus,
  RefreshCw,
  Settings2,
  ShieldCheck,
  TrendingUp,
  Wallet,
  X,
} from "lucide-react";
import { api, chooseFile, desktop, localDate } from "./bridge";
import type {
  AppState,
  Change,
  Editor,
  Filter,
  Legacy,
  Preview,
  Report,
  Consolidated,
} from "./types";
import { money, quantity, percent, sign, label } from "./format";
import { RecordForm } from "./Forms";
import { ImportWizard } from "./ImportWizard";
import {
  ActionBadge,
  InstrumentBadge,
  OutcomeText,
  PnL,
  StateBadge,
} from "./Semantics";

type Page =
  "Overview" | "Positions" | "Cycles" | "Cash" | "History" | "Data & source";
const pages = [
  { name: "Overview", icon: LayoutDashboard },
  { name: "Positions", icon: Layers3 },
  { name: "Cycles", icon: ArrowDownUp },
  { name: "Cash", icon: Wallet },
  { name: "History", icon: Clock3 },
  { name: "Data & source", icon: Database },
] as const;
function sourceLabel(s: AppState["source"]) {
  return s.kind === "local"
    ? s.path.split(/[\\/]/).pop()
    : s.kind === "google"
      ? "Google Sheets"
      : "Unsaved workspace";
}
function Metric({
  title,
  value,
  note,
  tone,
}: {
  title: string;
  value: string;
  note?: string;
  tone?: ReturnType<typeof sign>;
}) {
  return (
    <div className={`metric ${tone ?? "neutral"}`}>
      <span>{title}</span>
      <strong>
        {tone === undefined ? value : <OutcomeText value={value} tone={tone} />}
      </strong>
      {note && <small>{note}</small>}
    </div>
  );
}
export default function App() {
  const [state, setState] = useState<AppState>();
  const [filter, setFilter] = useState<Filter>({ as_of: localDate() });
  const [page, setPage] = useState<Page>("Overview");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [editor, setEditor] = useState<Editor>();
  const [preview, setPreview] = useState<Preview>();
  const [legacy, setLegacy] = useState<Legacy>();
  const [expanded, setExpanded] = useState<string>();
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [ccy, setCcy] = useState("USD");
  const [sourceId, setSourceId] = useState("");
  const [newName, setNewName] = useState("My portfolio");
  const [reportDate, setReportDate] = useState("2026-11-08");
  const [message, setMessage] = useState("");
  const load = useCallback(async () => {
    try {
      setState(await api<AppState>("state", { filter }));
    } catch (e) {
      setError(String(e));
    }
  }, [filter]);
  useEffect(() => {
    let active = true;
    api<AppState>("state", { filter })
      .then((s) => {
        if (active) setState(s);
      })
      .catch((e) => {
        if (active) setError(String(e));
      });
    return () => {
      active = false;
    };
  }, [filter]);
  async function run(action: string, payload: Record<string, unknown> = {}) {
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const result = await api<AppState>(action, { ...payload, filter });
      setState(result);
      if (
        [
          "sample",
          "new",
          "open_local",
          "open_google",
          "retry_pending",
        ].includes(action)
      ) {
        setFilter({ as_of: filter.as_of });
        setExpanded(undefined);
      }
      return result;
    } catch (e) {
      setError(String(e));
      await load();
      return undefined;
    } finally {
      setBusy(false);
    }
  }
  function openEditor(next: Editor) {
    setEditor(next);
    setPreview(undefined);
    setError("");
    setLegacy(undefined);
  }
  async function stage(
    changes: Change[],
    cycle?: string,
    action = "preview",
    extra: unknown = {},
  ) {
    if (!state) return;
    setBusy(true);
    setError("");
    try {
      setPreview(
        await api<Preview>(action, {
          changes,
          selected_cycle: cycle,
          expected_head: state.head,
          ...(extra as object),
          filter,
        }),
      );
    } catch (e) {
      setError(String(e));
      await load();
    } finally {
      setBusy(false);
    }
  }
  async function commit() {
    if (!preview) return;
    const result = await run("commit", { command_id: preview.command_id });
    if (result) {
      setPreview(undefined);
      setEditor(undefined);
      setLegacy(undefined);
      setMessage(
        result.source.kind === "sample"
          ? "Changes applied in memory. Save as XLSX or Google Sheets to keep this workspace."
          : "Saved and verified from the spreadsheet.",
      );
    }
  }
  async function fileAction(action: string) {
    try {
      const path = await chooseFile("xlsx", action !== "open_local");
      if (path) {
        const result = await run(action, { path });
        if (result)
          setMessage(
            action === "export"
              ? "Backup exported. The primary source is unchanged."
              : "Spreadsheet opened and verified.",
          );
      }
    } catch (e) {
      setError(String(e));
    }
  }
  async function importLegacy() {
    try {
      const path = await chooseFile("xlsx");
      if (!path) return;
      setBusy(true);
      setError("");
      const result = await api<Legacy>("import_preview", {
        path,
        as_of: reportDate,
      });
      setLegacy(result);
      setPreview(undefined);
      setEditor(undefined);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function importMarket() {
    try {
      const path = await chooseFile("csv");
      if (!path || !state) return;
      setBusy(true);
      setError("");
      setPreview(
        await api<Preview>("market_csv_preview", {
          path,
          expected_head: state.head,
          filter,
        }),
      );
      setEditor({ kind: "csv" });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  const p = state?.portfolio;
  const report = state?.report;
  const currencies = report?.totals.length
    ? report.totals.map((t) => t.currency)
    : [
        ...new Set(
          (p?.accounts ?? [])
            .filter((a) => !filter.account_id || a.id === filter.account_id)
            .flatMap((a) => a.currencies),
        ),
      ].sort();
  const currency = currencies.includes(ccy) ? ccy : (currencies[0] ?? "USD");
  const totals = report?.totals.find((t) => t.currency === currency);
  const performance = report?.performance.find((t) => t.currency === currency);
  const visible =
    report?.positions.filter(
      (v) =>
        v.instrument.currency === currency &&
        (!search ||
          `${v.instrument.symbol} ${v.position.id} ${v.position.name}`
            .toLowerCase()
            .includes(search.toLowerCase())) &&
        (status === "all" ||
          (status === "open"
            ? Number(v.quantity) > 0
            : Number(v.quantity) === 0)),
    ) ?? [];
  const edit = (kind: string) => openEditor({ kind });
  if (!state || !p || !report)
    return (
      <main className="loading">
        <div className="brand-symbol">C</div>
        <h1>Cycles</h1>
        <p>{error || "Opening portfolio…"}</p>
        {error && <button onClick={() => void load()}>Try again</button>}
      </main>
    );
  const hasData = !!p.accounts.length;
  const closeModal = () => {
    setEditor(undefined);
    setLegacy(undefined);
    setPreview(undefined);
    setError("");
  };
  return (
    <div className="app-shell">
      <aside className="sidebar" inert={busy || !!(editor || legacy)}>
        <div className="brand">
          <span className="brand-symbol">C</span>
          <span>
            Cycles<small>PORTFOLIO WORKSPACE</small>
          </span>
        </div>
        <nav>
          {pages.map(({ name, icon: Icon }) => (
            <button
              key={name}
              className={page === name ? "selected" : ""}
              onClick={() => setPage(name)}
            >
              <Icon size={18} />
              {name}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="source-badge">
            <FileSpreadsheet size={18} />
            <span>
              {sourceLabel(state.source)}
              <small>
                {state.source.kind === "sample"
                  ? "Save to keep your data"
                  : `Revision ${state.revision} · verified`}
              </small>
            </span>
          </div>
          <button className="subtle" onClick={() => edit("settings")}>
            <Settings2 size={16} />
            Preferences
          </button>
          <p>Each strategy has its own capital, positions, and history.</p>
        </div>
      </aside>
      <div className="main-shell" inert={busy || !!(editor || legacy)}>
        <header className="topbar">
          <div className="breadcrumb">
            WORKSPACE <ChevronRight size={13} /> {page}
          </div>
          <div className="top-actions">
            <span
              className={`save-state ${state.pending.length ? "pending" : ""}`}
            >
              <span className="dot" />
              {busy
                ? "Working…"
                : state.pending.length
                  ? "Pending save"
                  : state.source.kind === "sample"
                    ? "In memory"
                    : "Saved & verified"}
            </span>
            <button
              className="icon-button"
              title="Refresh spreadsheet"
              aria-label="Refresh spreadsheet"
              disabled={busy}
              onClick={() => void run("refresh")}
            >
              <RefreshCw size={17} />
            </button>
            <button
              onClick={() => void fileAction("export")}
              disabled={busy || !hasData}
            >
              <Download size={15} />
              Export XLSX
            </button>
          </div>
        </header>
        <main>
          <div className="page-heading">
            <div>
              <span className="eyebrow">YOUR INVESTMENT WORKSPACE</span>
              <h1>{page === "Overview" ? "Portfolio overview" : page}</h1>
              <p>
                {page === "Overview"
                  ? "See what your investments earned, and what you added."
                  : page === "Positions"
                    ? "Every position owns its transactions, basis, and profit."
                    : page === "Cycles"
                      ? "Keep each strategy’s capital and results separate."
                      : page === "Cash"
                        ? "Allocate capital deliberately. Transfers keep their ownership."
                        : page === "History"
                          ? "Dated evidence makes performance measurable."
                          : "Your spreadsheet is the source of truth."}
              </p>
            </div>
            {hasData && page !== "Data & source" && (
              <button
                className="primary"
                onClick={() =>
                  page === "Overview" || page === "Positions"
                    ? edit("position")
                    : page === "Cycles"
                      ? edit("cycle")
                      : page === "Cash"
                        ? edit("cash")
                        : edit("quote")
                }
              >
                <Plus size={16} />
                {page === "Cycles"
                  ? "New cycle"
                  : page === "Cash"
                    ? "Add movement"
                    : page === "History"
                      ? "Add quote"
                      : "Open position"}
              </button>
            )}
          </div>
          {!desktop && (
            <div className="notice">
              Read-only browser preview. All calculations shown here were
              produced by the Rust engine. Use the desktop app to save changes.
            </div>
          )}
          {error && !(editor || legacy) && (
            <div className="error" role="alert">
              {error}
            </div>
          )}
          {message && (
            <div className="success" role="status">
              {message}
            </div>
          )}
          {state.notice && <div className="notice">{state.notice}</div>}
          {!!state.pending.length && (
            <div className="pending-panel">
              <strong>Pending commands need attention</strong>
              <p>
                A write was interrupted or conflicted. Retry checks the original
                command ID before applying it.
              </p>
              {state.pending.map((x) => (
                <div key={x.id} className="inline">
                  <code>{x.id.slice(0, 8)}</code>
                  <span>
                    {sourceLabel(x.source)} · {x.created_at.slice(0, 16)}
                  </span>
                  <button
                    disabled={busy}
                    onClick={() => void run("retry_pending", { id: x.id })}
                  >
                    Retry
                  </button>
                  <button
                    disabled={busy}
                    onClick={async () => {
                      setBusy(true);
                      setError("");
                      try {
                        const pending = await api<Preview>("pending_preview", {
                          id: x.id,
                          filter,
                        });
                        await load();
                        setEditor({ kind: "pending" });
                        setLegacy(undefined);
                        setPreview(pending);
                      } catch (e) {
                        setError(String(e));
                      } finally {
                        setBusy(false);
                      }
                    }}
                  >
                    Review / rebase
                  </button>
                  <button
                    disabled={busy}
                    onClick={() => void run("discard_pending", { id: x.id })}
                  >
                    Discard intent
                  </button>
                </div>
              ))}
            </div>
          )}
          {!!state.conflicts.length && (
            <div className="notice">
              {state.conflicts.length} competing source command(s) were rejected
              during replay. Their records have no portfolio effect.
            </div>
          )}
          {hasData && page !== "Data & source" && (
            <div className="filterbar">
              <label>
                Cycle
                <select
                  disabled={!desktop}
                  aria-label="Cycle filter"
                  value={filter.cycle_id ?? ""}
                  onChange={(e) =>
                    setFilter({
                      ...filter,
                      cycle_id: e.target.value || undefined,
                    })
                  }
                >
                  <option value="">All cycles</option>
                  {p.cycles.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Account
                <select
                  disabled={!desktop}
                  aria-label="Account filter"
                  value={filter.account_id ?? ""}
                  onChange={(e) =>
                    setFilter({
                      ...filter,
                      account_id: e.target.value || undefined,
                    })
                  }
                >
                  <option value="">All accounts</option>
                  {p.accounts.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                From
                <input
                  disabled={!desktop}
                  aria-label="From date"
                  title="Optional. Leave empty to use all available history."
                  type="date"
                  value={filter.from ?? ""}
                  onChange={(e) =>
                    setFilter({ ...filter, from: e.target.value || undefined })
                  }
                />
              </label>
              <label>
                To
                <input
                  disabled={!desktop}
                  aria-label="To date"
                  title="Show portfolio values through this date."
                  type="date"
                  value={filter.as_of}
                  onChange={(e) =>
                    setFilter({ ...filter, as_of: e.target.value })
                  }
                />
              </label>
              <div
                className="currency-tabs"
                role="group"
                aria-label="Display currency"
              >
                {currencies.map((unit) => (
                  <button
                    className={currency === unit ? "active" : ""}
                    aria-pressed={currency === unit}
                    title={
                      page === "Positions"
                        ? `Show ${unit} positions in their native currency`
                        : undefined
                    }
                    key={unit}
                    onClick={() => setCcy(unit)}
                  >
                    {unit}
                  </button>
                ))}
              </div>
            </div>
          )}
          {!hasData && page !== "Data & source" ? (
            <section className="welcome panel">
              <div className="welcome-art">
                <Layers3 size={38} />
              </div>
              <span className="eyebrow">A CLEAR START</span>
              <h2>Your strategies. Their own story.</h2>
              <p>
                Create your first workspace or open an app spreadsheet.
                <br />
                You can import the existing tracker after choosing a source.
              </p>
              <div className="inline centered">
                <button
                  className="primary"
                  onClick={() => {
                    setPage("Data & source");
                  }}
                >
                  Set up workspace <ArrowUpRight size={16} />
                </button>
                <button onClick={() => void run("sample")}>
                  Explore sample
                </button>
              </div>
            </section>
          ) : (
            <>
              {page === "Overview" && (
                <>
                  {!totals && (
                    <div className="notice" role="status">
                      <strong>
                        No recorded activity for this date and selection.
                      </strong>
                      <br />
                      Choose a later end date or adjust the cycle/account
                      filters. Values remain unavailable until this view has
                      recorded data.
                    </div>
                  )}
                  <div className="metrics-grid">
                    <Metric
                      title="Portfolio equity"
                      value={money(totals?.equity, currency)}
                      note={
                        totals
                          ? `${totals.open_positions} open positions · ${currency}`
                          : `No activity in this view · ${currency}`
                      }
                    />
                    <Metric
                      title="Total position P/L"
                      value={money(totals?.total_pl, currency)}
                      note="Realized + unrealized · retained history"
                      tone={sign(totals?.total_pl)}
                    />
                    <Metric
                      title="Realized P/L"
                      value={money(totals?.realized, currency)}
                      note="Includes transaction fees"
                      tone={sign(totals?.realized)}
                    />
                    <Metric
                      title="Unrealized P/L"
                      value={money(totals?.unrealized, currency)}
                      note={
                        totals?.missing_quotes
                          ? `${totals?.missing_quotes} missing quote(s)`
                          : "Open positions at dated marks"
                      }
                      tone={sign(totals?.unrealized)}
                    />
                  </div>
                  <section className="panel chart-panel">
                    <div className="panel-heading">
                      <div>
                        <h2>Capital & growth</h2>
                        <p>
                          Asset gains are separated from net cash additions.
                        </p>
                      </div>
                      <span className="chip">
                        {currency} · {performance?.from ?? "No baseline"} →{" "}
                        {filter.as_of}
                      </span>
                    </div>
                    <div className="chart-legend">
                      <span>
                        <i className="equity-key" />
                        Portfolio equity
                      </span>
                      <span>
                        <i className="capital-key" />
                        Baseline + net contributions
                      </span>
                    </div>
                    {report.history.filter(
                      (h) => h.currency === currency && h.equity != null,
                    ).length > 1 ? (
                      <div className="chart">
                        <ResponsiveContainer width="100%" height={270}>
                          <ComposedChart
                            data={report.history
                              .filter((h) => h.currency === currency)
                              .map((h) => ({
                                ...h,
                                equity:
                                  h.equity == null ? null : Number(h.equity),
                                capital:
                                  h.capital == null ? null : Number(h.capital),
                              }))}
                            margin={{ top: 15, right: 15, bottom: 5, left: 15 }}
                          >
                            <defs>
                              <linearGradient
                                id="equityFill"
                                x1="0"
                                y1="0"
                                x2="0"
                                y2="1"
                              >
                                <stop
                                  offset="0"
                                  stopColor="var(--chart-equity)"
                                  stopOpacity={0.16}
                                />
                                <stop
                                  offset="1"
                                  stopColor="var(--chart-equity)"
                                  stopOpacity={0.01}
                                />
                              </linearGradient>
                            </defs>
                            <CartesianGrid
                              vertical={false}
                              stroke="var(--chart-grid)"
                            />
                            <XAxis
                              dataKey="date"
                              tickFormatter={(s) => s.slice(5)}
                              tickLine={false}
                              axisLine={false}
                              minTickGap={60}
                            />
                            <YAxis
                              tickFormatter={(v) =>
                                new Intl.NumberFormat("en", {
                                  notation: "compact",
                                }).format(v)
                              }
                              tickLine={false}
                              axisLine={false}
                            />
                            <Tooltip
                              cursor={{ stroke: "var(--chart-cursor)" }}
                              formatter={(value) =>
                                money(String(value), currency)
                              }
                            />
                            <Area
                              type="linear"
                              name="Equity"
                              dataKey="equity"
                              stroke="var(--chart-equity)"
                              fill="url(#equityFill)"
                              strokeWidth={2}
                              connectNulls={false}
                              isAnimationActive={false}
                            />
                            <Line
                              name="Capital"
                              dataKey="capital"
                              stroke="var(--chart-capital)"
                              dot={false}
                              strokeDasharray="5 5"
                              isAnimationActive={false}
                            />
                          </ComposedChart>
                        </ResponsiveContainer>
                      </div>
                    ) : (
                      <div className="empty chart-empty">
                        <TrendingUp size={30} />
                        <h3>History starts with dated evidence</h3>
                        <p>
                          Record an opening baseline, then add quotes and cash
                          movements.
                          <br />
                          Gaps stay visible; missing history is never invented.
                        </p>
                      </div>
                    )}
                  </section>
                  <div className="two-columns">
                    <section className="panel">
                      <div className="panel-heading">
                        <h2>Strategy performance</h2>
                        <span className="chip">{currency}</span>
                      </div>
                      <div className="performance-grid">
                        <Metric
                          title="Investment gain"
                          value={money(performance?.investment_gain, currency)}
                          tone={sign(performance?.investment_gain)}
                          note="Growth after contributions"
                        />
                        <Metric
                          title="Time-weighted return"
                          value={percent(performance?.twr)}
                          tone={sign(performance?.twr)}
                          note={performance?.twr_method}
                        />
                        <Metric
                          title="Money-weighted return"
                          value={percent(performance?.xirr)}
                          tone={sign(performance?.xirr)}
                          note={performance?.xirr_status}
                        />
                        <Metric
                          title="Portfolio volatility"
                          value={percent(performance?.volatility)}
                          note={`${performance?.observations ?? 0} valid daily returns · ${performance?.missing_periods ?? 0} gaps · annualized`}
                        />
                      </div>
                      {filter.from && (
                        <p className="hint">
                          Position P/L for the selected period:{" "}
                          <PnL value={totals?.period_pl} currency={currency} />.
                          This includes the change in unrealized value.
                        </p>
                      )}
                    </section>
                    <section className="panel">
                      <div className="panel-heading">
                        <h2>Exposure & data coverage</h2>
                        <ShieldCheck size={18} />
                      </div>
                      <dl className="facts">
                        <div>
                          <dt>Gross / net exposure</dt>
                          <dd>
                            {money(totals?.gross_exposure, currency)} /{" "}
                            {money(totals?.net_exposure, currency)}
                          </dd>
                        </div>
                        <div>
                          <dt>Planned loss at stops</dt>
                          <dd>{money(totals?.planned_risk, currency)}</dd>
                        </div>
                        <div>
                          <dt>Delta exposure</dt>
                          <dd>{money(totals?.delta_exposure, currency)}</dd>
                        </div>
                        <div>
                          <dt>Net external contributions</dt>
                          <dd>
                            {money(totals?.external_contributions, currency)}
                          </dd>
                        </div>
                        <div>
                          <dt>Net capital allocated to this scope</dt>
                          <dd>{money(totals?.contributed, currency)}</dd>
                        </div>
                        <div>
                          <dt>Cash balance</dt>
                          <dd>{money(totals?.cash, currency)}</dd>
                        </div>
                        <div>
                          <dt>Income / expenses</dt>
                          <dd>
                            {money(totals?.income, currency)} /{" "}
                            {money(totals?.expenses, currency)}
                          </dd>
                        </div>
                      </dl>
                      {totals && (
                        <p className="hint">
                          {totals.missing_stops} open positions without valid
                          stops · {totals.missing_deltas} without delta
                          evidence. Short exposure can have unbounded loss.
                        </p>
                      )}
                    </section>
                  </div>
                  <section className="panel section-gap">
                    <div className="panel-heading">
                      <h2>Allocation & concentration</h2>
                      <span className="chip">
                        Share of gross holdings · {currency}
                      </span>
                    </div>
                    <div className="table-wrap">
                      <table>
                        <thead>
                          <tr>
                            <th>Position</th>
                            <th>Signed market value</th>
                            <th>Gross allocation</th>
                          </tr>
                        </thead>
                        <tbody>
                          {report.allocation
                            ?.filter((x) => x.currency === currency)
                            .sort(
                              (a, b) =>
                                Number(b.gross_share ?? 0) -
                                Number(a.gross_share ?? 0),
                            )
                            .map((x) => (
                              <tr key={x.position_id}>
                                <td>
                                  {x.symbol}
                                  <small className="block">
                                    {x.position_id}
                                  </small>
                                </td>
                                <td>{money(x.signed_value, currency)}</td>
                                <td>
                                  {x.gross_share == null
                                    ? "Unavailable"
                                    : percent(Number(x.gross_share))}
                                </td>
                              </tr>
                            ))}
                        </tbody>
                      </table>
                    </div>
                    <p className="hint">
                      Each position’s absolute market value divided by gross
                      holdings in its currency. Cash is shown separately.
                    </p>
                  </section>
                  {report.attribution
                    ?.filter((x) => x.currency === currency)
                    .map((a) => (
                      <section className="panel section-gap" key={a.currency}>
                        <div className="panel-heading">
                          <h2>Investment gain reconciliation</h2>
                          <span className="chip">
                            {a.from ?? "No baseline"} → {a.to}
                          </span>
                        </div>
                        <dl className="facts">
                          <div>
                            <dt>Position P/L over this interval</dt>
                            <dd>
                              <PnL
                                value={a.period_position_pl}
                                currency={currency}
                              />
                            </dd>
                          </div>
                          <div>
                            <dt>Income minus expenses</dt>
                            <dd>
                              {money(a.income, currency)} −{" "}
                              {money(a.expenses, currency)}
                            </dd>
                          </div>
                          <div>
                            <dt>Baseline / basis / cash reconciliation</dt>
                            <dd>{money(a.reconciliation, currency)}</dd>
                          </div>
                          <div>
                            <dt>Total investment gain</dt>
                            <dd>
                              <PnL
                                value={a.investment_gain}
                                currency={currency}
                              />
                            </dd>
                          </div>
                        </dl>
                        <p className="hint">{a.note}</p>
                      </section>
                    ))}
                  {state.consolidated && (
                    <ConsolidationPanel data={state.consolidated} />
                  )}
                  {report.warnings.map((w) => (
                    <p className="notice" key={w}>
                      {w}
                    </p>
                  ))}
                </>
              )}
              {page === "Positions" && (
                <section className="panel position-panel">
                  <div className="table-toolbar">
                    <input
                      aria-label="Search positions"
                      placeholder="Search symbol or position ID…"
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                    />
                    <select
                      aria-label="Position status"
                      value={status}
                      onChange={(e) => setStatus(e.target.value)}
                    >
                      <option value="all">All positions</option>
                      <option value="open">Open</option>
                      <option value="closed">Closed</option>
                    </select>
                    <span className="muted" aria-live="polite">
                      {visible.length} {currency} positions
                    </span>
                  </div>
                  <div
                    className="color-legend"
                    aria-label="Position color guide"
                  >
                    <span>
                      <i className="legend-dot call-key" />
                      Call
                    </span>
                    <span>
                      <i className="legend-dot put-key" />
                      Put
                    </span>
                    <span>
                      <i className="legend-dot gain-key" />
                      Gain
                    </span>
                    <span>
                      <i className="legend-dot loss-key" />
                      Loss
                    </span>
                    <small>Type and status are separate from P/L.</small>
                  </div>
                  <div className="table-wrap">
                    <table className="positions">
                      <thead>
                        <tr>
                          <th>Position / cycle</th>
                          <th>Quantity</th>
                          <th>Avg. entry</th>
                          <th>Mark</th>
                          <th>Realized</th>
                          <th>Unrealized</th>
                          <th>Total P/L</th>
                          <th>Status</th>
                        </tr>
                      </thead>
                      <tbody>
                        {visible.map((v) => (
                          <PositionRows
                            key={v.position.id}
                            view={v}
                            expanded={expanded === v.position.id}
                            toggle={() =>
                              setExpanded(
                                expanded === v.position.id
                                  ? undefined
                                  : v.position.id,
                              )
                            }
                            edit={openEditor}
                            volatility={report.asset_volatility.find(
                              (x) => x.instrument_id === v.instrument.id,
                            )}
                          />
                        ))}
                      </tbody>
                    </table>
                  </div>
                  {!visible.length && (
                    <div className="empty">
                      <Layers3 size={28} />
                      <h3>No {currency} positions in this view</h3>
                      <p>Open a position or adjust the filters.</p>
                    </div>
                  )}
                </section>
              )}
              {page === "Cycles" && (
                <div className="cycle-list">
                  {p.cycles.map((c) => (
                    <section className="panel cycle-card" key={c.id}>
                      <div className="panel-heading">
                        <div>
                          <span className="eyebrow">
                            STARTED {c.start_date}
                          </span>
                          <h2>{c.name}</h2>
                        </div>
                        <StateBadge state={c.state} />
                      </div>
                      <p>
                        {c.strategy ||
                          "Add a strategy description to explain what you are testing."}
                      </p>
                      <div className="inline">
                        <span className="chip">
                          {
                            p.positions.filter((x) => x.cycle_id === c.id)
                              .length
                          }{" "}
                          positions owned
                        </span>
                        {c.stopped_on && (
                          <span className="chip">
                            Strategy stopped {c.stopped_on}
                          </span>
                        )}
                        {c.closed_on && (
                          <span className="chip">Settled {c.closed_on}</span>
                        )}
                      </div>
                      {p.snapshots
                        .filter(
                          (s) => s.cycle_id === c.id && s.phase === "stop",
                        )
                        .map((s) => (
                          <div className="list-row" key={s.id}>
                            <span>
                              At strategy stop · {String(s.date)}
                              <small className="block">
                                {
                                  p.accounts.find((a) => a.id === s.account_id)
                                    ?.name
                                }
                              </small>
                            </span>
                            <strong>
                              {money(String(s.equity), String(s.currency))}
                            </strong>
                          </div>
                        ))}
                      <div className="cycle-actions">
                        <button
                          onClick={() => {
                            setFilter({ ...filter, cycle_id: c.id });
                            setPage("Overview");
                          }}
                        >
                          View performance <ArrowUpRight size={14} />
                        </button>
                        <button
                          onClick={() => openEditor({ kind: "cycle", row: c })}
                        >
                          Edit strategy
                        </button>
                        {c.state !== "closed" && (
                          <button
                            disabled={busy}
                            onClick={() => {
                              setEditor({ kind: "cycle_state", row: c });
                              setPreview(undefined);
                              setError("");
                            }}
                          >
                            Finish strategy
                          </button>
                        )}
                      </div>
                    </section>
                  ))}
                </div>
              )}
              {page === "Cash" && (
                <>
                  <div className="panel">
                    <div className="panel-heading">
                      <h2>Cash by account & cycle</h2>
                      <button onClick={() => edit("reservation")}>
                        <Plus size={15} />
                        Reservation
                      </button>
                    </div>
                    <div className="table-wrap">
                      <table>
                        <thead>
                          <tr>
                            <th>Account / cycle</th>
                            <th>Currency</th>
                            <th>Cash</th>
                            <th>Reserved</th>
                            <th>Available to transfer</th>
                          </tr>
                        </thead>
                        <tbody>
                          {report.cash_balances.map((b) => (
                            <tr
                              key={`${b.account_id}|${b.cycle_id}|${b.currency}`}
                            >
                              <td>
                                {
                                  p.accounts.find((a) => a.id === b.account_id)
                                    ?.name
                                }
                                <small className="block">
                                  {
                                    p.cycles.find((c) => c.id === b.cycle_id)
                                      ?.name
                                  }
                                </small>
                              </td>
                              <td>{b.currency}</td>
                              <td>{money(b.balance, b.currency)}</td>
                              <td>{money(b.reserved, b.currency)}</td>
                              <td>
                                {money(b.available, b.currency)}
                                {b.reasons.map((x) => (
                                  <small className="block" key={x}>
                                    {x}
                                  </small>
                                ))}
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                  <section className="panel section-gap">
                    <div className="panel-heading">
                      <h2>Cash movements</h2>
                      <button onClick={() => edit("cash")}>
                        <Plus size={15} />
                        Add movement
                      </button>
                    </div>
                    <div className="table-wrap">
                      <table>
                        <thead>
                          <tr>
                            <th>Date</th>
                            <th>Movement</th>
                            <th>Account / cycle</th>
                            <th>Amount</th>
                            <th>Destination</th>
                            <th />
                          </tr>
                        </thead>
                        <tbody>
                          {p.cash_movements
                            .filter(
                              (m) =>
                                (!filter.cycle_id ||
                                  m.cycle_id === filter.cycle_id ||
                                  m.destination_cycle_id === filter.cycle_id) &&
                                (!filter.account_id ||
                                  m.account_id === filter.account_id ||
                                  m.destination_account_id ===
                                    filter.account_id),
                            )
                            .sort((a, b) => b.date.localeCompare(a.date))
                            .map((m) => (
                              <tr key={m.id}>
                                <td>{m.date}</td>
                                <td>
                                  {label(m.kind)}
                                  <small className="block">{m.notes}</small>
                                </td>
                                <td>
                                  {
                                    p.accounts.find(
                                      (a) => a.id === m.account_id,
                                    )?.name
                                  }
                                  <small className="block">
                                    {
                                      p.cycles.find((c) => c.id === m.cycle_id)
                                        ?.name
                                    }
                                  </small>
                                </td>
                                <td>{money(m.amount, m.currency)}</td>
                                <td>
                                  {p.cycles.find(
                                    (c) => c.id === m.destination_cycle_id,
                                  )?.name ??
                                    p.accounts.find(
                                      (a) => a.id === m.destination_account_id,
                                    )?.name ??
                                    (m.receive_amount
                                      ? money(
                                          m.receive_amount,
                                          m.receive_currency,
                                        )
                                      : "—")}
                                </td>
                                <td>
                                  <button
                                    className="subtle"
                                    onClick={() =>
                                      openEditor({ kind: "cash", row: m })
                                    }
                                  >
                                    Edit
                                  </button>
                                </td>
                              </tr>
                            ))}
                        </tbody>
                      </table>
                    </div>
                  </section>
                  <section className="panel section-gap">
                    <h2>Manual reservations</h2>
                    {p.reservations.map((r) => (
                      <div className="list-row" key={r.id}>
                        <span>
                          {String(
                            r.notes || r.position_id || "Cash reservation",
                          )}{" "}
                          · {money(String(r.amount), String(r.currency))}
                          <small className="block">
                            {String(r.start_date)} →{" "}
                            {String(r.end_date ?? "Not released")}
                          </small>
                        </span>
                        <button
                          onClick={() =>
                            openEditor({ kind: "reservation", row: r })
                          }
                        >
                          Edit / release
                        </button>
                      </div>
                    ))}
                  </section>
                </>
              )}
              {page === "History" && (
                <>
                  <div className="two-columns">
                    <section className="panel">
                      <h2>Market inputs</h2>
                      <p>
                        Dated quotes, option deltas, and underlying prices. No
                        automatic live price feed is connected.
                      </p>
                      <div className="inline">
                        <button onClick={() => edit("quote")}>
                          <Plus size={15} />
                          Quote
                        </button>
                        <button onClick={() => void importMarket()}>
                          Import quote CSV
                        </button>
                        <button onClick={() => edit("fx")}>FX rate</button>
                      </div>
                      <p className="hint">
                        CSV columns:
                        instrument_id,date,price,delta,underlying_price,adjusted,source.
                        Prices are per underlying unit.
                      </p>
                    </section>
                    <section className="panel">
                      <h2>Valuation evidence</h2>
                      <p>
                        Capture a dated close or an explicit boundary around a
                        cash flow. Every open asset needs a quote on the
                        selected date.
                      </p>
                      <SnapshotForm
                        busy={busy}
                        onPreview={(phase, flow_id, equity, currency) => {
                          setEditor({ kind: "snapshot" });
                          void stage([], undefined, "snapshot_preview", {
                            phase,
                            flow_id,
                            equity,
                            currency,
                          });
                        }}
                      />
                    </section>
                  </div>
                  <section className="panel section-gap">
                    <div className="panel-heading">
                      <h2>Quotes</h2>
                      <span className="muted">
                        {p.market_data.length} dated inputs
                      </span>
                    </div>
                    <div className="table-wrap capped">
                      <table>
                        <thead>
                          <tr>
                            <th>Date</th>
                            <th>Instrument</th>
                            <th>Price</th>
                            <th>Delta / underlying</th>
                            <th>Source</th>
                            <th />
                          </tr>
                        </thead>
                        <tbody>
                          {p.market_data
                            .slice()
                            .sort((a, b) => b.date.localeCompare(a.date))
                            .map((q) => (
                              <tr key={q.id}>
                                <td>{q.date}</td>
                                <td>
                                  {
                                    p.instruments.find(
                                      (i) => i.id === q.instrument_id,
                                    )?.symbol
                                  }
                                </td>
                                <td>
                                  {money(
                                    q.price,
                                    p.instruments.find(
                                      (i) => i.id === q.instrument_id,
                                    )?.currency,
                                  )}
                                </td>
                                <td>
                                  {q.delta ?? "—"} / {q.underlying_price ?? "—"}
                                </td>
                                <td>
                                  {q.source}
                                  {q.adjusted ? " · adjusted" : ""}
                                </td>
                                <td>
                                  <button
                                    className="subtle"
                                    onClick={() =>
                                      openEditor({ kind: "quote", row: q })
                                    }
                                  >
                                    Edit
                                  </button>
                                </td>
                              </tr>
                            ))}
                        </tbody>
                      </table>
                    </div>
                  </section>
                  <section className="panel section-gap">
                    <h2>Recorded valuations & FX</h2>
                    {p.snapshots.map((s) => (
                      <div className="list-row" key={s.id}>
                        <span>
                          {String(s.date)} · {String(s.phase)} ·{" "}
                          {String(s.cycle_id ?? "All cycles")}
                        </span>
                        <strong>
                          {money(String(s.equity), String(s.currency))}
                        </strong>
                      </div>
                    ))}
                    {p.fx_rates.map((r) => (
                      <div className="list-row" key={r.id}>
                        <span>
                          {String(r.date)} · {String(r.from)} → {String(r.to)} ·{" "}
                          {String(r.rate)}
                        </span>
                        <button
                          onClick={() => openEditor({ kind: "fx", row: r })}
                        >
                          Edit
                        </button>
                      </div>
                    ))}
                  </section>
                </>
              )}
              {page === "Data & source" && (
                <>
                  <div className="two-columns">
                    <section className="panel">
                      <FileSpreadsheet className="section-icon" />
                      <h2>Local spreadsheet</h2>
                      <p>
                        Use an app-controlled XLSX as your primary data file.
                        Saves verify the records and preserve the previous file.
                      </p>
                      <div className="inline">
                        <button
                          className="primary"
                          onClick={() => void fileAction("open_local")}
                        >
                          Open XLSX
                        </button>
                        <button
                          disabled={!hasData}
                          onClick={() => void fileAction("save_as")}
                        >
                          Save workspace as XLSX
                        </button>
                      </div>
                      <p className="hint">
                        {state.source.kind === "local"
                          ? state.source.path
                          : "No local primary file selected."}
                      </p>
                    </section>
                    <section className="panel">
                      <Database className="section-icon" />
                      <h2>Google Sheets</h2>
                      <p>
                        Sign in through your system browser. Credentials stay in
                        the OS credential store.
                      </p>
                      <div className="inline">
                        <button
                          disabled={busy}
                          onClick={async () => {
                            try {
                              const path = await chooseFile("json");
                              if (path) {
                                setBusy(true);
                                await api("authorize_google", { path });
                                await load();
                              }
                            } catch (e) {
                              setError(String(e));
                            } finally {
                              setBusy(false);
                            }
                          }}
                        >
                          {state.google_connected
                            ? "Reconnect Google"
                            : "Connect Google"}
                        </button>
                        <span className="chip">
                          {state.google_connected
                            ? "Connected"
                            : "Desktop OAuth client required"}
                        </span>
                      </div>
                      <label className="field">
                        <span>Spreadsheet title</span>
                        <input
                          value={newName}
                          onChange={(e) => setNewName(e.target.value)}
                        />
                      </label>
                      <button
                        disabled={busy || !hasData || !state.google_connected}
                        onClick={() =>
                          void run("create_google", { title: newName })
                        }
                      >
                        Create Sheets source from workspace
                      </button>
                      <label className="field">
                        <span>App-created spreadsheet ID or URL</span>
                        <input
                          value={sourceId}
                          onChange={(e) => setSourceId(e.target.value)}
                        />
                      </label>
                      <button
                        disabled={busy || !sourceId || !state.google_connected}
                        onClick={() =>
                          void run("open_google", { id: sourceId })
                        }
                      >
                        Open Sheets source
                      </button>
                      <p className="hint">
                        The drive.file scope permits app-created documents.
                        Import other spreadsheets through a local XLSX copy.
                      </p>
                    </section>
                  </div>
                  <section className="panel section-gap">
                    <h2>Start or explore a workspace</h2>
                    <div className="inline">
                      <label className="field">
                        <span>First cycle starts</span>
                        <input
                          type="date"
                          value={filter.as_of}
                          onChange={(e) =>
                            setFilter({ ...filter, as_of: e.target.value })
                          }
                        />
                      </label>
                      <button
                        disabled={busy}
                        onClick={() =>
                          void run("new", {
                            date: filter.as_of,
                            cycle: "First cycle",
                            account: "Brokerage",
                          })
                        }
                      >
                        <Plus size={15} />
                        New empty workspace
                      </button>
                      <button
                        disabled={busy}
                        onClick={() => void run("sample")}
                      >
                        Load sample
                      </button>
                    </div>
                    <p className="hint">
                      An unsaved workspace lives in memory. Save it to XLSX or
                      create a Google Sheets source before relying on it.
                    </p>
                  </section>
                  <section className="panel section-gap">
                    <h2>Import your existing tracker</h2>
                    <p>
                      Preview raw positions and transactions, choose their
                      owning cycles, and reconcile report-date cash. The
                      original tracker remains intact.
                    </p>
                    <div className="inline">
                      <label className="field">
                        <span>Source report date</span>
                        <input
                          type="date"
                          value={reportDate}
                          onChange={(e) => setReportDate(e.target.value)}
                        />
                      </label>
                      <button
                        disabled={busy || !hasData}
                        onClick={() => void importLegacy()}
                      >
                        Choose tracker XLSX & preview
                      </button>
                    </div>
                    <p className="hint">
                      Bank and credit-card statement tabs are not imported.
                      Unsupported commodity records require explicit exclusion.
                      Imported P/L covers retained known-basis history.
                    </p>
                  </section>
                  <section className="panel section-gap">
                    <div className="panel-heading">
                      <h2>Accounts</h2>
                      <button onClick={() => edit("account")}>
                        <Plus size={15} />
                        Add account
                      </button>
                    </div>
                    {p.accounts.map((a) => (
                      <div className="list-row" key={a.id}>
                        <span>
                          {a.name}
                          <small className="block">
                            {a.broker} · {a.currencies.join(" / ")}
                          </small>
                        </span>
                        <button
                          onClick={() =>
                            openEditor({ kind: "account", row: a })
                          }
                        >
                          Edit
                        </button>
                      </div>
                    ))}
                  </section>
                  <p className="hint">
                    Dataset {p.dataset_id} · {p.transactions.length}{" "}
                    transactions · no spreadsheet formulas · accounting policy
                    v1
                  </p>
                </>
              )}
            </>
          )}
        </main>
        <footer>
          <span>Cycles · Spreadsheet-backed portfolio accounting</span>
          <span>Position ID ownership · Weighted-average basis</span>
        </footer>
      </div>
      {(editor || legacy) && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="dialog-title"
            onKeyDown={(e) => {
              if (e.key === "Escape" && !busy) {
                e.stopPropagation();
                closeModal();
              }
              if (e.key === "Tab") {
                const fields = Array.from(
                  e.currentTarget.querySelectorAll<HTMLElement>(
                    "button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),summary",
                  ),
                ).filter((x) => x.offsetParent !== null);
                const first = fields[0],
                  last = fields.at(-1);
                if (e.shiftKey && document.activeElement === first) {
                  e.preventDefault();
                  last?.focus();
                } else if (!e.shiftKey && document.activeElement === last) {
                  e.preventDefault();
                  first?.focus();
                }
              }
            }}
          >
            <div className="modal-heading">
              <div>
                <span className="eyebrow">PREVIEW BEFORE SAVE</span>
                <h2 id="dialog-title">
                  {legacy
                    ? "Import tracker"
                    : editor?.kind === "transaction"
                      ? `${editor.row ? "Edit" : "Add"} transaction · ${editor.position?.instrument.symbol}`
                      : editor?.kind === "position"
                        ? "Open position"
                        : label(editor?.kind ?? "")}
                </h2>
              </div>
              <button
                className="icon-button"
                autoFocus
                disabled={busy}
                aria-label="Close editor"
                onClick={closeModal}
              >
                <X size={22} />
              </button>
            </div>
            {error && (
              <div className="error" role="alert">
                {error}
              </div>
            )}
            {legacy ? (
              <ImportWizard
                legacy={legacy}
                p={p}
                busy={busy}
                onDirty={() => setPreview(undefined)}
                onPrepare={(choices) =>
                  void stage([], undefined, "import_prepare", { choices })
                }
              />
            ) : editor?.kind === "cycle_state" ? (
              <div>
                <p>
                  Strategy activity ends on {filter.as_of}. Open positions
                  remain owned by this cycle until settled. Closure requires
                  every position to be flat.
                </p>
                <div className="inline">
                  <button
                    disabled={busy}
                    onClick={() =>
                      void stage([], String(editor.row?.id), "cycle_preview", {
                        id: editor.row?.id,
                        state: "winding_down",
                        date: filter.as_of,
                      })
                    }
                  >
                    Preview winding down
                  </button>
                  <button
                    disabled={busy}
                    onClick={() =>
                      void stage([], String(editor.row?.id), "cycle_preview", {
                        id: editor.row?.id,
                        state: "closed",
                        date: filter.as_of,
                      })
                    }
                  >
                    Preview settled closure
                  </button>
                </div>
              </div>
            ) : (
              editor &&
              !["csv", "snapshot", "pending"].includes(editor.kind) && (
                <RecordForm
                  key={`${editor.kind}:${editor.row?.id ?? editor.position?.position.id ?? "new"}`}
                  editor={editor}
                  p={p}
                  filter={filter}
                  busy={busy}
                  onDirty={() => setPreview(undefined)}
                  onPreview={(...args) => void stage(...args)}
                />
              )
            )}
            {preview && (
              <PreviewPanel
                preview={preview}
                current={report}
                busy={busy}
                inMemory={state.source.kind === "sample"}
                onSave={() => void commit()}
              />
            )}
          </section>
        </div>
      )}
    </div>
  );
}

function PositionRows({
  view: v,
  expanded,
  toggle,
  edit,
  volatility,
}: {
  view: Report["positions"][number];
  expanded: boolean;
  toggle: () => void;
  edit: (e: Editor) => void;
  volatility?: Report["asset_volatility"][number];
}) {
  const c = v.instrument.currency;
  return (
    <>
      <tr className="position-row" onClick={toggle}>
        <td>
          <button className="position-toggle" aria-expanded={expanded}>
            {expanded ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
            <span>
              <span className="position-name">
                <strong>{v.instrument.symbol}</strong>
                <InstrumentBadge
                  asset={v.instrument.asset}
                  right={v.instrument.option_right}
                />
              </span>
              <small>
                <span
                  className={`direction-label direction-${v.position.direction}`}
                >
                  {label(v.position.direction)}
                </span>{" "}
                · {v.cycle_name}
              </small>
            </span>
          </button>
        </td>
        <td>{quantity(v.quantity)}</td>
        <td>{money(v.average_entry, c)}</td>
        <td>
          {money(v.price, c)}
          <small
            className={`block ${v.stale || !v.quote_date ? "caution" : ""}`}
          >
            {v.quote_date ?? "Missing quote"}
            {v.stale ? " · stale" : ""}
          </small>
        </td>
        <td>
          <PnL value={v.realized} currency={c} />
        </td>
        <td>
          <PnL value={v.unrealized} currency={c} />
        </td>
        <td className="pnl-total">
          <PnL value={v.total_pl} currency={c} />
        </td>
        <td>
          <StateBadge state={v.status} />
        </td>
      </tr>
      {expanded && (
        <tr className="expanded-row">
          <td colSpan={8}>
            <div className="position-detail">
              <div className="panel-heading">
                <div>
                  <strong>{v.position.name || v.instrument.symbol}</strong>
                  <p>
                    {v.account_name} · ID {v.position.id}
                  </p>
                </div>
                <div className="inline">
                  <button
                    onClick={() =>
                      edit({
                        kind: "position_settings",
                        row: v.position,
                        position: v,
                      })
                    }
                  >
                    Risk / notes
                  </button>
                  <button
                    className="primary"
                    onClick={() => edit({ kind: "transaction", position: v })}
                  >
                    <Plus size={14} />
                    Add transaction
                  </button>
                  {v.instrument.asset === "option" &&
                    Number(v.quantity) > 0 && (
                      <button
                        onClick={() =>
                          edit({ kind: "settlement", position: v })
                        }
                      >
                        Exercise / assignment
                      </button>
                    )}
                </div>
              </div>
              <div className="detail-metrics">
                <span>
                  Remaining basis <strong>{money(v.basis, c)}</strong>
                </span>
                <span>
                  Planned stop risk <strong>{money(v.planned_risk, c)}</strong>
                </span>
                <span>
                  Maximum loss{" "}
                  <strong className={v.unbounded ? "caution" : undefined}>
                    {v.unbounded ? "⚠ Unbounded" : money(v.maximum_loss, c)}
                  </strong>
                </span>
                <span>
                  Delta exposure <strong>{money(v.delta_exposure, c)}</strong>
                </span>
                <span>
                  Asset volatility{" "}
                  <strong>{percent(volatility?.volatility)}</strong>
                  <small>
                    {volatility?.observations ?? 0} returns ·{" "}
                    {volatility?.method}
                  </small>
                </span>
              </div>
              {v.instrument.asset === "option" && (
                <p className="hint">
                  {label(v.instrument.option_right ?? "")} · strike{" "}
                  {money(v.instrument.strike, c)} · expiry {v.instrument.expiry}{" "}
                  · multiplier {quantity(v.instrument.multiplier)} ·{" "}
                  {label(v.instrument.settlement)} settlement
                </p>
              )}
              <div className="table-wrap">
                <table className="transactions">
                  <thead>
                    <tr>
                      <th>Date / order</th>
                      <th>Action</th>
                      <th>Quantity</th>
                      <th>Unit price</th>
                      <th>Fee</th>
                      <th>Realized P/L</th>
                      <th>Remaining basis</th>
                      <th />
                    </tr>
                  </thead>
                  <tbody>
                    {v.ledger.map((l) => (
                      <tr key={l.transaction.id}>
                        <td>
                          {l.transaction.date}
                          <small className="block">
                            #{l.transaction.order}
                          </small>
                        </td>
                        <td>
                          <ActionBadge action={l.transaction.action} />
                          {l.transaction.settlement_id && (
                            <small className="block">Linked settlement</small>
                          )}
                        </td>
                        <td>{quantity(l.transaction.quantity)}</td>
                        <td>{money(l.transaction.price, c)}</td>
                        <td>{money(l.transaction.fee, c)}</td>
                        <td>
                          <PnL value={l.realized} currency={c} />
                        </td>
                        <td>{money(l.basis_after, c)}</td>
                        <td>
                          {!l.transaction.settlement_id &&
                            !["assignment", "exercise"].includes(
                              l.transaction.action,
                            ) && (
                              <button
                                className="subtle"
                                onClick={() =>
                                  edit({
                                    kind: "transaction",
                                    row: l.transaction,
                                    position: v,
                                  })
                                }
                              >
                                Edit
                              </button>
                            )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              {v.position.notes && <p className="hint">{v.position.notes}</p>}
            </div>
          </td>
        </tr>
      )}
    </>
  );
}
function SnapshotForm({
  busy,
  onPreview,
}: {
  busy: boolean;
  onPreview: (
    phase: string,
    flow: string,
    equity: string,
    currency: string,
  ) => void;
}) {
  const [phase, setPhase] = useState("close");
  const [flow, setFlow] = useState("");
  const [equity, setEquity] = useState("");
  const [currency, setCurrency] = useState("USD");
  return (
    <div className="inline">
      <select
        aria-label="Valuation phase"
        value={phase}
        onChange={(e) => setPhase(e.target.value)}
      >
        {["close", "baseline", "pre_flow", "post_flow"].map((x) => (
          <option key={x} value={x}>
            {label(x)}
          </option>
        ))}
      </select>
      {phase.includes("flow") && (
        <input
          placeholder="Cash movement ID"
          aria-label="Cash flow ID"
          value={flow}
          onChange={(e) => setFlow(e.target.value)}
        />
      )}
      {phase.includes("flow") && (
        <>
          <input
            aria-label="Measured flow-time equity"
            placeholder="Measured flow-time equity"
            type="number"
            step="any"
            value={equity}
            onChange={(e) => setEquity(e.target.value)}
          />
          <input
            aria-label="Flow-time currency"
            placeholder="Currency"
            value={currency}
            onChange={(e) => setCurrency(e.target.value.toUpperCase())}
          />
          <p className="hint wide">
            Enter the measured total equity immediately before or after this
            cash flow. A daily closing value alone is not flow-time evidence.
          </p>
        </>
      )}
      <button
        disabled={
          busy || (phase.includes("flow") && (!flow || !equity || !currency))
        }
        onClick={() => onPreview(phase, flow, equity, currency)}
      >
        Preview valuation
      </button>
    </div>
  );
}
function PreviewPanel({
  preview,
  current,
  busy,
  inMemory,
  onSave,
}: {
  preview: Preview;
  current: Report;
  busy: boolean;
  inMemory: boolean;
  onSave: () => void;
}) {
  return (
    <section className="preview">
      <h3>Review the complete command</h3>
      <p>
        {preview.changes.length} record revision(s). Nothing is saved until this
        command is accepted and read back.
      </p>
      {!!preview.prior_records?.length && (
        <details>
          <summary>
            Inspect current stored records before replacing them
          </summary>
          {preview.prior_records.map((r, i) => (
            <pre key={i}>
              {r.table} · {r.record_id}
              {"\n"}
              {JSON.stringify(r.payload, null, 2)}
            </pre>
          ))}
        </details>
      )}
      <div className="preview-records">
        {preview.changes.map((x, i) => (
          <details key={`${x.table}:${x.record_id}:${i}`}>
            <summary>
              {x.table} · {x.record_id}
            </summary>
            <pre>{JSON.stringify(x.payload, null, 2)}</pre>
          </details>
        ))}
      </div>
      <table>
        <thead>
          <tr>
            <th>Currency</th>
            <th>Equity before → after</th>
            <th>Total position P/L after</th>
          </tr>
        </thead>
        <tbody>
          {preview.report.totals.map((t) => (
            <tr key={t.currency}>
              <td>{t.currency}</td>
              <td>
                {money(
                  current.totals.find((x) => x.currency === t.currency)?.equity,
                  t.currency,
                )}{" "}
                → {money(t.equity, t.currency)}
              </td>
              <td>{money(t.total_pl, t.currency)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {preview.report.positions
        .filter((v) =>
          preview.changes.some((c) =>
            c.table === "Positions"
              ? c.record_id === v.position.id
              : c.table === "Transactions" &&
                (c.payload as Record<string, unknown> | null)?.position_id ===
                  v.position.id,
          ),
        )
        .map((v) => (
          <div className="notice" key={v.position.id}>
            <span className="position-name">
              <strong>{v.instrument.symbol}</strong>
              <InstrumentBadge
                asset={v.instrument.asset}
                right={v.instrument.option_right}
              />
            </span>{" "}
            · remaining {quantity(v.quantity)} · basis{" "}
            {money(v.basis, v.instrument.currency)} · realized{" "}
            {money(v.realized, v.instrument.currency)} · total P/L{" "}
            {money(v.total_pl, v.instrument.currency)}
          </div>
        ))}
      {!!preview.cash_after?.length && (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Cash bucket</th>
                <th>Before → after</th>
                <th>Reserved / available after</th>
              </tr>
            </thead>
            <tbody>
              {preview.cash_after
                .filter((b) => {
                  const before = preview.cash_before?.find(
                    (a) =>
                      a.account_id === b.account_id &&
                      a.cycle_id === b.cycle_id &&
                      a.currency === b.currency,
                  );
                  return (
                    !before ||
                    String(before.balance) !== String(b.balance) ||
                    String(before.reserved) !== String(b.reserved)
                  );
                })
                .map((b) => {
                  const before = preview.cash_before?.find(
                    (a) =>
                      a.account_id === b.account_id &&
                      a.cycle_id === b.cycle_id &&
                      a.currency === b.currency,
                  );
                  return (
                    <tr key={`${b.account_id}|${b.cycle_id}|${b.currency}`}>
                      <td>
                        {b.account_id}
                        <small className="block">
                          {b.cycle_id} · {b.currency}
                        </small>
                      </td>
                      <td>
                        {money(before?.balance, b.currency)} →{" "}
                        {money(b.balance, b.currency)}
                      </td>
                      <td>
                        {money(b.reserved, b.currency)} /{" "}
                        {money(b.available, b.currency)}
                      </td>
                    </tr>
                  );
                })}
            </tbody>
          </table>
        </div>
      )}
      <div className="inline">
        <button className="primary" disabled={busy} onClick={onSave}>
          {busy
            ? "Verifying…"
            : inMemory
              ? "Apply in memory"
              : "Save to spreadsheet"}
        </button>
        <small>
          {inMemory
            ? "Save workspace as XLSX or Google Sheets afterward."
            : "On conflict your draft stays here for review."}
        </small>
      </div>
    </section>
  );
}

function ConsolidationPanel({ data: c }: { data: Consolidated }) {
  return (
    <section className="panel section-gap">
      <div className="panel-heading">
        <h2>Consolidated portfolio · {c.currency}</h2>
        <span className="chip">
          {c.performance.from ?? "No supported baseline"} → {c.performance.to}
        </span>
      </div>
      <div className="metrics-grid">
        <Metric title="Current equity" value={money(c.equity, c.currency)} />
        <Metric
          title="Investment gain"
          value={money(c.performance.investment_gain, c.currency)}
          tone={sign(c.performance.investment_gain)}
        />
        <Metric
          title="Time-weighted return"
          value={percent(c.performance.twr)}
          tone={sign(c.performance.twr)}
          note={c.performance.twr_method}
        />
        <Metric
          title="Money-weighted return"
          value={percent(c.performance.xirr)}
          tone={sign(c.performance.xirr)}
          note={c.performance.xirr_status}
        />
      </div>
      {c.history.filter((h) => h.equity != null).length > 1 && (
        <ResponsiveContainer width="100%" height={230}>
          <ComposedChart
            data={c.history.map((h) => ({
              ...h,
              equity: h.equity == null ? null : Number(h.equity),
              capital: h.capital == null ? null : Number(h.capital),
            }))}
          >
            <CartesianGrid vertical={false} stroke="var(--chart-grid)" />
            <XAxis
              dataKey="date"
              tickFormatter={(d) => d.slice(5)}
              minTickGap={50}
            />
            <YAxis
              tickFormatter={(v) =>
                new Intl.NumberFormat("en", { notation: "compact" }).format(v)
              }
            />
            <Tooltip
              cursor={{ stroke: "var(--chart-cursor)" }}
              formatter={(v) => money(String(v), c.currency)}
            />
            <Line
              dataKey="equity"
              name="Equity"
              stroke="var(--chart-equity)"
              dot={false}
              isAnimationActive={false}
            />
            <Line
              dataKey="capital"
              name="Capital"
              stroke="var(--chart-capital)"
              strokeDasharray="5 5"
              dot={false}
              isAnimationActive={false}
            />
          </ComposedChart>
        </ResponsiveContainer>
      )}
      <dl className="facts">
        <div>
          <dt>Position P/L in reporting currency</dt>
          <dd>
            <PnL value={c.position_pl} currency={c.currency} />
          </dd>
        </div>
        <div>
          <dt>Income / expenses at event-date FX</dt>
          <dd>
            {money(c.income, c.currency)} / {money(c.expenses, c.currency)}
          </dd>
        </div>
        <div>
          <dt>Native baseline / basis / cash reconciliation</dt>
          <dd>{money(c.reconciliation, c.currency)}</dd>
        </div>
        <div>
          <dt>Currency translation & conversion spread</dt>
          <dd>{money(c.currency_effect, c.currency)}</dd>
        </div>
        <div>
          <dt>Portfolio volatility</dt>
          <dd>
            {percent(c.performance.volatility)} · {c.performance.observations}{" "}
            returns · {c.performance.missing_periods} gaps
          </dd>
        </div>
      </dl>
      <p className="hint">
        Equity uses valuation-date FX; capital and realized P/L use event-date
        FX. Unrealized P/L uses each boundary’s FX. Native reconciliation
        differences are converted at ending FX before the remaining currency
        effect is identified. Returns with cash flows use the labelled daily
        convention.
      </p>
      {c.warnings.map((w) => (
        <p className="notice" key={w}>
          {w}
        </p>
      ))}
    </section>
  );
}
