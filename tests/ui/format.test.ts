import { expect, it } from "vitest";
import { money } from "../../src/format";
it("displays decimal minor units without a binary float round trip", () => {
  expect(money("9007199254740993.005", "USD")).toBe(
    "US$9,007,199,254,740,993.01",
  );
  expect(money("-1.005", "CAD")).toBe("-$1.01");
  expect(money("1.004", "CAD")).toBe("$1.00");
  expect(money(null)).toBe("Unavailable");
});
