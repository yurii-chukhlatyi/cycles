import {
  render,
  screen,
  fireEvent,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import App from "../../src/App";
import { RecordForm } from "../../src/Forms";
import { ImportWizard } from "../../src/ImportWizard";
import type { AppState, Change, Legacy } from "../../src/types";
import fixture from "../../public/preview.json";
const call = vi.hoisted(() => vi.fn());
vi.mock("../../src/bridge", async () => ({
  api: call,
  desktop: true,
  localDate: () => "2026-11-09",
  uid: () => crypto.randomUUID(),
  chooseFile: vi.fn(),
}));
const state = fixture as unknown as AppState;
beforeEach(() => {
  call.mockReset();
  call.mockImplementation(async (action: string) => {
    if (action === "commit")
      throw Error("Workbook changed; your draft is retained");
    if (action === "preview")
      return { command_id: "command", changes: [], report: state.report };
    return state;
  });
});
describe("portfolio entry workflows", () => {
  it("keeps a failed transaction draft and scopes it to its position cycle", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: "Portfolio overview" });
    await user.click(screen.getByRole("button", { name: "Positions" }));
    await user.click(screen.getByRole("button", { name: /ACME/ }));
    await user.click(screen.getByRole("button", { name: "Add transaction" }));
    const dialog = screen.getByRole("dialog");
    fireEvent.change(within(dialog).getByLabelText("Action"), {
      target: { value: "close" },
    });
    fireEvent.change(within(dialog).getByLabelText("Transaction date"), {
      target: { value: "2026-11-06" },
    });
    fireEvent.change(within(dialog).getByLabelText("Quantity"), {
      target: { value: "7" },
    });
    await user.click(
      within(dialog).getByRole("button", { name: "Preview changes" }),
    );
    await user.click(
      await within(dialog).findByRole("button", { name: "Apply in memory" }),
    );
    await within(dialog).findByRole("alert");
    expect(within(dialog).getByLabelText("Transaction date")).toHaveValue(
      "2026-11-06",
    );
    expect(within(dialog).getByLabelText("Quantity")).toHaveValue(7);
    const payload = call.mock.calls.find((x) => x[0] === "preview")![1];
    expect(payload.selected_cycle).toBe("cycle");
    expect(payload.changes[0].value.position_id).toBe("pos");
    expect(payload.changes[0].value.cycle_id).toBe("cycle");
  });
  it("creates a position and opening transaction in one command without multiplying option prices", async () => {
    const user = userEvent.setup();
    const submit = vi.fn();
    const p = state.portfolio;
    render(
      <RecordForm
        editor={{ kind: "position" }}
        p={p}
        filter={{ as_of: "2026-11-09", cycle_id: "cycle" }}
        onPreview={submit}
        onDirty={() => {}}
        busy={false}
      />,
    );
    fireEvent.change(screen.getByLabelText("Symbol"), {
      target: { value: "TEST CALL" },
    });
    fireEvent.change(screen.getByLabelText("Asset type"), {
      target: { value: "option" },
    });
    fireEvent.change(screen.getByLabelText("Underlying symbol"), {
      target: { value: "TEST" },
    });
    fireEvent.change(screen.getByLabelText("Strike"), {
      target: { value: "120" },
    });
    fireEvent.change(screen.getByLabelText("Expiry"), {
      target: { value: "2026-12-18" },
    });
    fireEvent.change(screen.getByLabelText("Price per underlying unit"), {
      target: { value: "2" },
    });
    await user.click(screen.getByRole("button", { name: "Preview changes" }));
    await waitFor(() => expect(submit).toHaveBeenCalled());
    const changes = submit.mock.calls[0][0] as Change[];
    expect(changes.map((x) => x.table)).toEqual([
      "Instruments",
      "Positions",
      "Transactions",
    ]);
    expect(changes[2].value).toMatchObject({
      price: "2",
      cycle_id: "cycle",
      date: "2026-11-09",
    });
    expect(changes[0].value).toMatchObject({ multiplier: "100" });
  });
});

it("locks a transaction draft while its matching preview is pending", async () => {
  const user = userEvent.setup();
  let finish!: (v: unknown) => void;
  call.mockImplementation(async (action: string) =>
    action === "preview"
      ? new Promise((resolve) => {
          finish = resolve;
        })
      : state,
  );
  render(<App />);
  await screen.findByRole("heading", { name: "Portfolio overview" });
  await user.click(screen.getByRole("button", { name: "Positions" }));
  await user.click(screen.getByRole("button", { name: /ACME/ }));
  await user.click(screen.getByRole("button", { name: "Add transaction" }));
  const dialog = screen.getByRole("dialog");
  await user.click(
    within(dialog).getByRole("button", { name: "Preview changes" }),
  );
  expect(within(dialog).getByLabelText("Quantity")).toBeDisabled();
  expect(screen.getByRole("button", { name: "Close editor" })).toBeDisabled();
  finish({ command_id: "locked", changes: [], report: state.report });
  await waitFor(() =>
    expect(within(dialog).getByLabelText("Quantity")).toBeEnabled(),
  );
});
it("clears incompatible cycle filters when another dataset is selected", async () => {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("heading", { name: "Portfolio overview" });
  fireEvent.change(screen.getByLabelText("Cycle filter"), {
    target: { value: "cycle" },
  });
  await user.click(screen.getByRole("button", { name: "Data & source" }));
  const changed = {
    ...state,
    portfolio: {
      ...state.portfolio,
      cycles: [
        { ...state.portfolio.cycles[0], id: "new-cycle", name: "New cycle" },
      ],
    },
  };
  call.mockImplementation(async () => changed);
  await user.click(screen.getByRole("button", { name: "Load sample" }));
  await user.click(screen.getByRole("button", { name: "Overview" }));
  expect(await screen.findByLabelText("Cycle filter")).toHaveValue("");
});

it("keeps report-date baseline anchors out of new transaction sequencing", () => {
  const p = structuredClone(state.portfolio);
  p.cash_movements.push({
    id: "baseline",
    account_id: "account",
    cycle_id: "cycle",
    currency: "USD",
    date: "2026-11-08",
    kind: "baseline",
    amount: "1000",
    notes: "",
    order: 2147483647,
  });
  render(
    <RecordForm
      editor={{ kind: "transaction", position: state.report.positions[0] }}
      p={p}
      filter={{ as_of: "2026-11-08" }}
      onPreview={vi.fn()}
      onDirty={vi.fn()}
      busy={false}
    />,
  );
  expect(screen.getByLabelText("Order within date")).toHaveValue(1);
});

it("requires explicit import cash and locks the reviewed allocation during preparation", async () => {
  const user = userEvent.setup();
  const submit = vi.fn();
  const legacy: Legacy = {
    portfolio: state.portfolio,
    source_hash: "fixture",
    as_of: "2026-11-09",
    earliest_date: "2026-01-02",
    unsupported: ["Unsupported commodity"],
    pending_settlements: [],
    warnings: [],
    cash_controls: { USD: "1000" },
    reported_quantities: { pos: "80" },
  };
  const props = {
    legacy,
    p: state.portfolio,
    onPrepare: submit,
    onDirty: vi.fn(),
  };
  const { rerender } = render(<ImportWizard {...props} busy={false} />);
  expect(screen.getByRole("button", { name: "Preview import" })).toBeDisabled();
  fireEvent.change(screen.getByRole("spinbutton"), { target: { value: "0" } });
  await user.click(screen.getByRole("checkbox"));
  await user.click(screen.getByRole("button", { name: "Preview import" }));
  expect(submit.mock.calls[0][0]).toMatchObject({
    acknowledge_exclusions: true,
    cash: [{ amount: "0" }],
    position_cycles: { pos: expect.any(String) },
  });
  rerender(<ImportWizard {...props} busy={true} />);
  expect(screen.getByRole("spinbutton")).toBeDisabled();
  expect(screen.getByRole("checkbox")).toBeDisabled();
  for (const select of screen.getAllByRole("combobox"))
    expect(select).toBeDisabled();
});
