import { browser, $ } from "@wdio/globals";
const { describe, it } = globalThis;
import assert from "node:assert/strict";
import { resolve } from "node:path";
import { writeFileSync, readFileSync, existsSync, unlinkSync } from "node:fs";
const filter = { as_of: "2026-11-09" };
async function api(action, payload = {}) {
  return browser.tauri.execute(
    async ({ core }, action, payload) =>
      core.invoke("api", { action, payload }),
    action,
    { ...payload, filter },
  );
}
async function verified() {
  await $('[aria-label="Refresh spreadsheet"]').click();
  await $('[aria-label="Refresh spreadsheet"]').waitForEnabled({
    timeout: 30000,
  });
}
async function previewSave() {
  await $("button=Preview changes").click();
  const save = await $("button=Save to spreadsheet");
  await save.waitForEnabled({ timeout: 30000 });
  await save.click();
  await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 30000 });
}
function assertDecimal(actual, expected) {
  const canonical = (s) =>
    s.includes(".") ? s.replace(/0+$/, "").replace(/\.$/, "") : s;
  assert.match(actual, /^-?\d+(\.\d+)?$/);
  assert.equal(canonical(actual), canonical(expected));
}
function comparable(a, b) {
  if (typeof a === "number" && typeof b === "number") {
    assert.ok(Math.abs(a - b) <= 1e-10 * Math.max(1, Math.abs(a), Math.abs(b)));
    return;
  }
  if (a && b && typeof a === "object" && typeof b === "object") {
    assert.deepEqual(Object.keys(a).sort(), Object.keys(b).sort());
    for (const k of Object.keys(a)) comparable(a[k], b[k]);
    return;
  }
  assert.deepEqual(a, b);
}
describe("native portfolio workflows", () => {
  it("saves a partial close, isolates a new cycle, and restores a complete backup", async () => {
    await browser.waitUntil(async () => await $("h1").isExisting(), {
      timeout: 30000,
    });
    let state = await api("sample");
    const source = resolve("test-results/native/primary.xlsx");
    const backup = resolve("test-results/native/backup.xlsx");
    for (const path of [source, backup]) if (existsSync(path)) unlinkSync(path);
    state = await api("save_as", { path: source });
    await verified();
    await $("button=Positions").click();
    await $("button.position-toggle").click();
    await $("button=Add transaction").click();
    // The embedded 1.5.1 driver clicks <option> with JS click(), which does
    // not select it in WebKit. Dispatch the real control change in this webview.
    await browser.execute(
      (control) => {
        control.value = "close";
        control.dispatchEvent(new Event("change", { bubbles: true }));
      },
      await $('select[name="action"]'),
    );
    assert.equal(await $('select[name="action"]').getValue(), "close");
    await $('input[name="date"]').setValue("2026-11-09");
    await $('input[name="quantity"]').setValue("5");
    await $('input[name="price"]').setValue("125");
    await $('input[name="fee"]').setValue("1");
    await previewSave();
    state = await api("state");
    const position = state.report.positions.find(
      (p) => p.position.id === "pos",
    );
    assert.equal(position.quantity, "75");
    assertDecimal(position.realized, "502.50");
    assertDecimal(position.total_pl, "2166.00");
    assert.match(await $("body").getText(), /Saved and verified/);
    const cycle = {
      id: "native-next",
      name: "Next strategy",
      strategy: "Independent capital",
      start_date: "2026-11-09",
      state: "active",
    };
    let preview = await api("preview", {
      expected_head: state.head,
      changes: [{ table: "Cycles", id: cycle.id, value: cycle }],
    });
    state = await api("commit", { command_id: preview.command_id });
    const transfer = {
      id: "native-transfer",
      account_id: "account",
      cycle_id: "cycle",
      currency: "USD",
      date: "2026-11-09",
      order: 100,
      kind: "cycle_transfer",
      amount: "1000",
      destination_cycle_id: cycle.id,
      notes: "Native test transfer",
    };
    preview = await api("preview", {
      expected_head: state.head,
      selected_cycle: "cycle",
      changes: [{ table: "CashMovements", id: transfer.id, value: transfer }],
    });
    assert.equal(
      preview.cash_after.find(
        (b) => b.cycle_id === cycle.id && b.currency === "USD",
      ).balance,
      "1000",
    );
    state = await api("commit", { command_id: preview.command_id });
    assert.equal(state.portfolio.positions[0].cycle_id, "cycle");
    assert.equal(
      state.portfolio.positions.filter((p) => p.cycle_id === cycle.id).length,
      0,
    );
    const total = state.report.totals.find((t) => t.currency === "USD");
    assert.equal(total.external_contributions, "30000");
    preview = await api("cycle_preview", {
      expected_head: state.head,
      id: "cycle",
      state: "winding_down",
      date: "2026-11-09",
    });
    state = await api("commit", { command_id: preview.command_id });
    assert.equal(
      state.portfolio.cycles.find((c) => c.id === "cycle").state,
      "winding_down",
    );
    await assert.rejects(
      () =>
        api("preview", {
          expected_head: state.head,
          selected_cycle: cycle.id,
          changes: [
            {
              table: "Transactions",
              id: "wrong-cycle",
              value: {
                id: "wrong-cycle",
                position_id: "pos",
                cycle_id: cycle.id,
                date: "2026-11-09",
                order: 101,
                action: "close",
                quantity: "1",
                price: "125",
                fee: "0",
              },
            },
          ],
        }),
      /cycle|belongs/i,
    );
    await api("export", { path: backup });
    const before = await api("state");
    const restored = await api("open_local", { path: backup });
    comparable(before.portfolio, restored.portfolio);
    comparable(before.report, restored.report);
    writeFileSync(
      resolve("test-results/native/expected.json"),
      JSON.stringify({
        portfolio: restored.portfolio,
        report: restored.report,
      }),
    );
    const invalid = resolve("test-results/native/invalid.xlsx");
    writeFileSync(invalid, "invalid workbook fixture");
    await assert.rejects(() => api("open_local", { path: invalid }));
    assert.equal((await api("state")).head, restored.head);
    await verified();
    await $("button=Overview").click();
    await browser.saveScreenshot(resolve("test-results/native/overview.png"));
  });
  it("can restore another platform’s accepted workbook without calculation drift", async function () {
    if (
      !process.env.CYCLES_RESTORE_FIXTURE ||
      !process.env.CYCLES_RESTORE_EXPECTED
    ) {
      this.skip();
      return;
    }
    const expected = JSON.parse(
      readFileSync(process.env.CYCLES_RESTORE_EXPECTED, "utf8"),
    );
    const state = await api("open_local", {
      path: resolve(process.env.CYCLES_RESTORE_FIXTURE),
    });
    comparable(expected.portfolio, state.portfolio);
    comparable(expected.report, state.report);
    await verified();
  });
});
