import { useState } from "react";
import type { Change, Editor, Filter, Portfolio } from "./types";
import { localDate, uid } from "./bridge";
import { label } from "./format";

type Fields = Record<string, string>;
type Options = { value: string; label: string }[];
export function Field({
  name,
  title,
  value,
  change,
  type = "text",
  options,
  optional = false,
  help,
}: {
  name: string;
  title: string;
  value: string;
  change: (k: string, v: string) => void;
  type?: string;
  options?: Options;
  optional?: boolean;
  help?: string;
}) {
  return (
    <label className="field">
      <span>{title}</span>
      {options ? (
        <select
          name={name}
          value={value}
          onChange={(e) => change(name, e.target.value)}
          required={!optional}
        >
          {optional && <option value="">None</option>}
          {options.map((x) => (
            <option key={x.value} value={x.value}>
              {x.label}
            </option>
          ))}
        </select>
      ) : (
        <input
          name={name}
          type={type}
          step="any"
          required={!optional}
          value={value}
          onChange={(e) => change(name, e.target.value)}
        />
      )}
      {help && <small>{help}</small>}
    </label>
  );
}
export function RecordForm({
  editor,
  p,
  filter,
  onPreview,
  onDirty,
  busy,
}: {
  editor: Editor;
  p: Portfolio;
  filter: Filter;
  onPreview: (
    changes: Change[],
    cycle?: string,
    action?: string,
    extra?: unknown,
  ) => void;
  onDirty: () => void;
  busy: boolean;
}) {
  const r = editor.row;
  const pos = editor.position;
  const defaultCycle =
    pos?.position.cycle_id ??
    filter.cycle_id ??
    p.cycles.find((c) => c.state === "active")?.id ??
    p.cycles[0]?.id ??
    "";
  const [f, setF] = useState<Fields>(() => ({
    id: r?.id ?? uid(),
    cycle_id: defaultCycle,
    account_id:
      pos?.position.account_id ?? filter.account_id ?? p.accounts[0]?.id ?? "",
    currency: pos?.instrument.currency ?? filter.currency ?? "USD",
    currencies: "USD,CAD",
    instrument_id: p.instruments[0]?.id ?? "",
    adjusted: "false",
    destination_cycle_id: p.cycles.find((c) => c.id !== defaultCycle)?.id ?? "",
    destination_account_id: p.accounts[1]?.id ?? "",
    receive_currency: "CAD",
    date: filter.as_of || localDate(),
    order: String(
      1 +
        Math.max(
          0,
          ...p.transactions
            .filter((t) => t.date === filter.as_of)
            .map((t) => t.order),
          ...p.cash_movements
            .filter((m) => m.date === filter.as_of && m.kind !== "baseline")
            .map((m) => Number(m.order ?? 0)),
        ),
    ),
    quantity: editor.kind === "settlement" && pos ? String(pos.quantity) : "1",
    price: "0",
    fee: "0",
    action: "open",
    asset: "stock",
    direction: "long",
    multiplier: "100",
    option_right: "call",
    settlement: "physical",
    start_date: filter.as_of || localDate(),
    kind: "deposit",
    amount: "0",
    from: "USD",
    to: "CAD",
    rate: "1",
    volatility_window: String(p.settings.volatility_window),
    timezone: p.settings.timezone,
    calendar: p.settings.calendar,
    reporting_currency: p.settings.reporting_currency ?? "",
    holidays: p.settings.holidays.join(","),
    ...Object.fromEntries(
      Object.entries(r ?? {}).map(([k, v]) => [
        k,
        v == null ? "" : Array.isArray(v) ? v.join(",") : String(v),
      ]),
    ),
  }));
  const change = (k: string, v: string) => {
    if (busy) return;
    setF({ ...f, [k]: v });
    onDirty();
  };
  const field = (
    name: string,
    title: string,
    type = "text",
    options?: Options,
    optional = false,
    help?: string,
  ) => (
    <Field
      key={name}
      {...{ name, title, type, options, optional, help }}
      value={f[name] ?? ""}
      change={change}
    />
  );
  const cycles = p.cycles.map((c) => ({
    value: c.id,
    label: `${c.name} · ${label(c.state)}`,
  }));
  const accounts = p.accounts.map((a) => ({ value: a.id, label: a.name }));
  const options = (xs: string[]) =>
    xs.map((x) => ({ value: x, label: label(x) }));
  const scope = (
    <>
      {field(
        "cycle_id",
        "Owning cycle",
        "text",
        editor.kind === "position"
          ? cycles.filter(
              (c) => p.cycles.find((x) => x.id === c.value)?.state === "active",
            )
          : cycles,
      )}
      {field("account_id", "Account", "text", accounts)}
      {field(
        "currency",
        "Currency",
        "text",
        options(
          p.accounts.find((a) => a.id === f.account_id)?.currencies ?? [
            "USD",
            "CAD",
          ],
        ),
      )}
    </>
  );
  const up = (table: string, value: unknown, id = f.id): Change => ({
    table,
    id,
    value,
  });
  function submit(e: React.FormEvent) {
    e.preventDefault();
    const own = f.cycle_id;
    let changes: Change[] = [];
    switch (editor.kind) {
      case "position": {
        const instrument = uid();
        const transaction = uid();
        changes = [
          up(
            "Instruments",
            {
              id: instrument,
              symbol: f.symbol?.trim().toUpperCase(),
              asset: f.asset,
              currency: f.currency,
              multiplier: f.asset === "option" ? f.multiplier : "1",
              option_right: f.asset === "option" ? f.option_right : null,
              strike: f.asset === "option" ? f.strike : null,
              expiry: f.asset === "option" ? f.expiry : null,
              underlying:
                f.asset === "option" ? f.underlying?.toUpperCase() : null,
              settlement: f.settlement,
              deliverable: f.deliverable ?? "",
            },
            instrument,
          ),
          up("Positions", {
            id: f.id,
            instrument_id: instrument,
            account_id: f.account_id,
            cycle_id: own,
            direction: f.direction,
            name: f.name ?? "",
            stop_price: f.stop_price || null,
            covered_by: f.covered_by || null,
            notes: f.notes ?? "",
            pending_lifecycle: false,
            source: "",
          }),
          up(
            "Transactions",
            {
              id: transaction,
              position_id: f.id,
              cycle_id: own,
              date: f.date,
              order: Number(f.order),
              action: "open",
              quantity: f.quantity,
              price: f.price,
              fee: f.fee,
              notes: "",
            },
            transaction,
          ),
        ];
        break;
      }
      case "transaction":
        changes = [
          up("Transactions", {
            ...r,
            id: f.id,
            position_id: pos!.position.id,
            cycle_id: own,
            date: f.date,
            order: Number(f.order),
            action: f.action,
            quantity: f.quantity,
            price: f.price,
            fee: f.fee,
            basis_total: f.basis_total || null,
            notes: f.notes ?? "",
          }),
        ];
        break;
      case "position_settings":
        changes = [
          up("Positions", {
            ...r,
            stop_price: f.stop_price || null,
            covered_by: f.covered_by || null,
            name: f.name ?? "",
            notes: f.notes ?? "",
          }),
        ];
        break;
      case "settlement":
        onPreview([], own, "settle_preview", {
          request: {
            position_id: pos!.position.id,
            date: f.date,
            quantity: f.quantity,
            fee: f.fee,
            cash_price: f.cash_price || null,
            underlying_position_id: f.underlying_position_id || null,
            order: Number(f.order),
          },
        });
        return;
      case "cash":
        changes = [
          up("CashMovements", {
            ...r,
            id: f.id,
            cycle_id: own,
            account_id: f.account_id,
            currency: f.currency,
            date: f.date,
            order: Number(f.order),
            kind: f.kind,
            amount: f.amount,
            destination_cycle_id:
              f.kind === "cycle_transfer"
                ? f.destination_cycle_id || null
                : null,
            destination_account_id:
              f.kind === "account_transfer"
                ? f.destination_account_id || null
                : null,
            receive_currency:
              f.kind === "fx" ? f.receive_currency || null : null,
            receive_amount: f.kind === "fx" ? f.receive_amount || null : null,
            notes: f.notes ?? "",
          }),
        ];
        break;
      case "cycle":
        changes = [
          up("Cycles", {
            ...r,
            id: f.id,
            name: f.name,
            strategy: f.strategy ?? "",
            start_date: f.start_date,
            state: r?.state ?? "active",
          }),
        ];
        break;
      case "account":
        changes = [
          up("Accounts", {
            ...r,
            id: f.id,
            name: f.name,
            broker: f.broker ?? "",
            currencies: f.currencies
              .split(",")
              .map((s) => s.trim().toUpperCase())
              .filter(Boolean),
          }),
        ];
        break;
      case "quote":
        changes = [
          up("MarketData", {
            ...r,
            id: f.id,
            instrument_id: f.instrument_id,
            date: f.date,
            price: f.price,
            delta: f.delta || null,
            underlying_price: f.underlying_price || null,
            adjusted: f.adjusted === "true",
            source: f.source || "Manual",
          }),
        ];
        break;
      case "fx":
        changes = [
          up("FxRates", {
            ...r,
            id: f.id,
            from: f.from,
            to: f.to,
            date: f.date,
            rate: f.rate,
            source: f.source || "Manual",
          }),
        ];
        break;
      case "reservation":
        changes = [
          up("Reservations", {
            ...r,
            id: f.id,
            cycle_id: own,
            account_id: f.account_id,
            currency: f.currency,
            amount: f.amount,
            start_date: f.start_date,
            end_date: f.end_date || null,
            position_id: f.position_id || null,
            notes: f.notes ?? "",
          }),
        ];
        break;
      case "settings":
        changes = [
          up(
            "Settings",
            {
              ...p.settings,
              reporting_currency: f.reporting_currency || null,
              volatility_window: Number(f.volatility_window),
              holidays: f.holidays
                .split(",")
                .map((x) => x.trim())
                .filter(Boolean),
              calendar: f.calendar,
              timezone: f.timezone,
            },
            "settings",
          ),
        ];
        break;
    }
    onPreview(changes, own);
  }
  return (
    <form onSubmit={submit} className="editor-form">
      <fieldset disabled={busy} className="form-fields">
        <div className="form-grid">
          {["position", "cash", "reservation"].includes(editor.kind) && scope}
          {editor.kind === "position" && (
            <>
              {field("symbol", "Symbol")}
              {field(
                "asset",
                "Asset type",
                "text",
                options(["stock", "option"]),
              )}
              {field(
                "direction",
                "Direction",
                "text",
                options(["long", "short"]),
              )}
              {field("name", "Position label", "text", undefined, true)}
              {f.asset === "option" && (
                <>
                  {field("underlying", "Underlying symbol")}
                  {field(
                    "option_right",
                    "Call / put",
                    "text",
                    options(["call", "put"]),
                  )}
                  {field("strike", "Strike", "number")}
                  {field("expiry", "Expiry", "date")}
                  {field("multiplier", "Contract multiplier", "number")}
                  {field(
                    "settlement",
                    "Settlement",
                    "text",
                    options(["physical", "cash"]),
                  )}
                  {field(
                    "deliverable",
                    "Deliverable details",
                    "text",
                    undefined,
                    true,
                  )}
                </>
              )}
              {field("stop_price", "Stop price", "number", undefined, true)}
              {field(
                "covered_by",
                "Covered by position",
                "text",
                p.positions
                  .filter(
                    (x) =>
                      x.cycle_id === ownCycle(f) &&
                      x.account_id === f.account_id &&
                      x.direction === "long",
                  )
                  .map((x) => ({
                    value: x.id,
                    label:
                      p.instruments.find((i) => i.id === x.instrument_id)
                        ?.symbol ?? x.id,
                  })),
                true,
              )}
            </>
          )}
          {["position", "transaction", "settlement"].includes(editor.kind) && (
            <>
              {editor.kind === "transaction" &&
                field(
                  "action",
                  "Action",
                  "text",
                  options([
                    "open",
                    "close",
                    "expiry",
                    "basis",
                    "snapshot",
                    "split",
                  ]),
                )}
              {field("date", "Transaction date", "date")}
              {field("order", "Order within date", "number")}
              {field(
                "quantity",
                pos?.instrument.asset === "option" || f.asset === "option"
                  ? "Contracts"
                  : "Quantity",
                "number",
              )}
              {editor.kind !== "settlement" &&
                field("price", "Price per underlying unit", "number")}
              {field("fee", "Total fee", "number")}
              {f.action === "basis" &&
                field(
                  "basis_total",
                  "New total basis",
                  "number",
                  undefined,
                  true,
                  "Quantity and fee must be zero for a basis adjustment.",
                )}
              {editor.kind === "settlement" && (
                <>
                  {pos?.instrument.settlement === "cash" &&
                    field(
                      "cash_price",
                      "Cash settlement price per unit",
                      "number",
                    )}
                  {field(
                    "underlying_position_id",
                    "Reduce existing underlying position",
                    "text",
                    p.positions
                      .filter(
                        (x) =>
                          x.cycle_id === defaultCycle &&
                          x.account_id === pos?.position.account_id &&
                          p.instruments.find((i) => i.id === x.instrument_id)
                            ?.asset === "stock",
                      )
                      .map((x) => ({
                        value: x.id,
                        label: `${p.instruments.find((i) => i.id === x.instrument_id)?.symbol} · ${x.direction}`,
                      })),
                    true,
                  )}
                </>
              )}
            </>
          )}
          {editor.kind === "position_settings" && (
            <>
              {field("name", "Position label", "text", undefined, true)}
              {field("stop_price", "Stop price", "number", undefined, true)}
              {field(
                "covered_by",
                "Covered by position ID",
                "text",
                undefined,
                true,
              )}
            </>
          )}
          {editor.kind === "cash" && (
            <>
              {field(
                "kind",
                "Movement type",
                "text",
                options([
                  "initial",
                  "deposit",
                  "withdrawal",
                  "cycle_transfer",
                  "account_transfer",
                  "fx",
                  "dividend",
                  "interest",
                  "expense",
                  "baseline",
                ]),
              )}
              {field("date", "Date", "date")}
              {field("order", "Order within date", "number")}
              {field("amount", "Amount in source currency", "number")}
              {f.kind === "cycle_transfer" &&
                field(
                  "destination_cycle_id",
                  "Destination cycle",
                  "text",
                  cycles,
                )}
              {f.kind === "account_transfer" &&
                field(
                  "destination_account_id",
                  "Destination account",
                  "text",
                  accounts,
                )}
              {f.kind === "fx" && (
                <>
                  {field("receive_currency", "Receive currency")}
                  {field("receive_amount", "Amount received", "number")}
                </>
              )}
              {f.kind === "baseline" && (
                <p className="wide hint">
                  A baseline replaces reconstructed cash through this date. Use
                  only for reconciled opening balances.
                </p>
              )}
            </>
          )}
          {editor.kind === "cycle" && (
            <>
              {field("name", "Cycle name")}
              {field("start_date", "Start date", "date")}
              {field(
                "strategy",
                "Strategy description",
                "text",
                undefined,
                true,
              )}
            </>
          )}
          {editor.kind === "account" && (
            <>
              {field("name", "Account name")}
              {field("broker", "Broker / reference", "text", undefined, true)}
              {field(
                "currencies",
                "Enabled currencies",
                "text",
                undefined,
                false,
                "Comma separated, e.g. USD,CAD",
              )}
            </>
          )}
          {editor.kind === "quote" && (
            <>
              {field(
                "instrument_id",
                "Instrument",
                "text",
                p.instruments.map((i) => ({
                  value: i.id,
                  label: `${i.symbol} · ${i.currency}`,
                })),
              )}
              {field("date", "Quote date", "date")}
              {field("price", "Price per underlying unit", "number")}
              {field(
                "delta",
                "Option delta (−1 to 1)",
                "number",
                undefined,
                true,
              )}
              {field(
                "underlying_price",
                "Underlying price",
                "number",
                undefined,
                true,
              )}
              {field(
                "adjusted",
                "Adjusted price series",
                "text",
                options(["false", "true"]),
              )}
              {field("source", "Source", "text", undefined, true)}
            </>
          )}
          {editor.kind === "fx" && (
            <>
              {field("from", "From currency")}
              {field("to", "To currency")}
              {field("date", "Rate date", "date")}
              {field("rate", "Units of To per 1 From", "number")}
              {field("source", "Source", "text", undefined, true)}
            </>
          )}
          {editor.kind === "reservation" && (
            <>
              {field("amount", "Reserved amount", "number")}
              {field("start_date", "Start date", "date")}
              {field("end_date", "Release date", "date", undefined, true)}
              {field("position_id", "Position ID", "text", undefined, true)}
            </>
          )}
          {editor.kind === "settings" && (
            <>
              {field(
                "reporting_currency",
                "Optional reporting currency",
                "text",
                undefined,
                true,
              )}
              {field(
                "volatility_window",
                "Volatility lookback (20–252)",
                "number",
              )}
              {field(
                "holidays",
                "Market holidays, YYYY-MM-DD",
                "text",
                undefined,
                true,
              )}
              {field("calendar", "Calendar label")}
              {field("timezone", "Time zone")}
            </>
          )}
          {!["settings", "cycle", "quote", "fx", "account"].includes(
            editor.kind,
          ) && field("notes", "Notes", "text", undefined, true)}
        </div>
        {editor.kind === "settlement" && (
          <p className="hint">
            The preview includes the option close and matching shares at the
            strike, in the owning cycle. Any excess delivery opens a new
            underlying position.
          </p>
        )}
        <button className="primary" disabled={busy}>
          Preview changes
        </button>
      </fieldset>
    </form>
  );
}
function ownCycle(f: Fields) {
  return f.cycle_id;
}
