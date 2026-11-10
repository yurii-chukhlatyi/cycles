import { useState } from "react";
import type { Legacy, Portfolio } from "./types";
import { uid } from "./bridge";
import { money } from "./format";
export function ImportWizard({
  legacy,
  p,
  busy,
  onPrepare,
  onDirty,
}: {
  legacy: Legacy;
  p: Portfolio;
  busy: boolean;
  onPrepare: (choices: unknown) => void;
  onDirty: () => void;
}) {
  const [cycle] = useState(() => ({
    ...legacy.portfolio.cycles[0],
    id: uid(),
  }));
  const [mapping, setMapping] = useState<Record<string, string>>(() =>
    Object.fromEntries(legacy.portfolio.positions.map((x) => [x.id, cycle.id])),
  );
  const [cash, setCash] = useState<Record<string, string>>({});
  const [excluded, setExcluded] = useState(false);
  const [settled, setSettled] = useState(false);
  const cycles = [cycle, ...p.cycles];
  const buckets = Array.from(
    new Set(
      legacy.portfolio.positions.map((pos) => {
        const ccy = legacy.portfolio.instruments.find(
          (i) => i.id === pos.instrument_id,
        )!.currency;
        return `${pos.account_id}|${mapping[pos.id]}|${ccy}`;
      }),
    ),
  );
  function prepare() {
    onPrepare({
      position_cycles: mapping,
      cycles: [cycle],
      cash: buckets.map((key) => {
        const [account_id, cycle_id, currency] = key.split("|");
        return { account_id, cycle_id, currency, amount: cash[key] ?? "0" };
      }),
      acknowledge_exclusions: excluded,
      reconcile_historical_settlements: settled,
    });
  }
  return (
    <fieldset disabled={busy} className="import-wizard form-fields">
      <p>
        <strong>
          {legacy.portfolio.positions.length} positions ·{" "}
          {legacy.portfolio.transactions.length} transactions
        </strong>
        <br />
        Source report date {legacy.as_of}. First retained transaction{" "}
        {legacy.earliest_date}.
      </p>
      {legacy.warnings.map((w, i) => (
        <p className="notice" key={i}>
          {w}
        </p>
      ))}
      <h3>Assign positions to their permanent cycles</h3>
      <p className="hint">
        A separate historical cycle is created by default. Each position keeps
        its original ID.
      </p>
      <div className="scroll-table">
        <table>
          <thead>
            <tr>
              <th>Position</th>
              <th>Account</th>
              <th>Cycle</th>
            </tr>
          </thead>
          <tbody>
            {legacy.portfolio.positions.map((pos) => (
              <tr key={pos.id}>
                <td>
                  {
                    legacy.portfolio.instruments.find(
                      (i) => i.id === pos.instrument_id,
                    )?.symbol
                  }
                  <small className="block">{pos.id}</small>
                </td>
                <td>
                  {
                    legacy.portfolio.accounts.find(
                      (a) => a.id === pos.account_id,
                    )?.name
                  }
                </td>
                <td>
                  <select
                    aria-label={`Cycle for ${pos.id}`}
                    value={mapping[pos.id]}
                    onChange={(e) => {
                      setMapping({ ...mapping, [pos.id]: e.target.value });
                      onDirty();
                    }}
                  >
                    {cycles.map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.name}
                      </option>
                    ))}
                  </select>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <h3>Reconcile opening cash</h3>
      <p className="hint">
        Allocate the report's cash across account and cycle buckets. Enter zero
        explicitly when a bucket has no cash. The app verifies each currency
        total.
      </p>
      <p>
        {Object.entries(legacy.cash_controls).map(([ccy, value]) => (
          <strong className="chip" key={ccy}>
            {money(value, ccy)} control
          </strong>
        ))}
      </p>
      <div className="form-grid">
        {buckets.map((key) => {
          const [a, c, ccy] = key.split("|");
          return (
            <label className="field" key={key}>
              <span>
                {legacy.portfolio.accounts.find((x) => x.id === a)?.name} ·{" "}
                {cycles.find((x) => x.id === c)?.name} · {ccy}
              </span>
              <input
                aria-label={`Opening cash ${key}`}
                type="number"
                step="any"
                required
                value={cash[key] ?? ""}
                onChange={(e) => {
                  setCash({ ...cash, [key]: e.target.value });
                  onDirty();
                }}
              />
            </label>
          );
        })}
      </div>
      {!!legacy.unsupported.length && (
        <label className="check">
          <input
            type="checkbox"
            checked={excluded}
            onChange={(e) => {
              setExcluded(e.target.checked);
              onDirty();
            }}
          />
          Exclude unsupported records: {legacy.unsupported.join(", ")}. They
          remain in the original workbook.
        </label>
      )}
      {!!legacy.pending_settlements.length && (
        <label className="check">
          <input
            type="checkbox"
            checked={settled}
            onChange={(e) => {
              setSettled(e.target.checked);
              onDirty();
            }}
          />
          I reconciled the report-date holdings and cash for historical
          assignments without underlying links. Earlier settlement detail
          remains incomplete.
        </label>
      )}
      <button
        className="primary"
        disabled={
          busy || buckets.some((key) => cash[key] == null || cash[key] === "")
        }
        onClick={prepare}
      >
        Preview import
      </button>
    </fieldset>
  );
}
