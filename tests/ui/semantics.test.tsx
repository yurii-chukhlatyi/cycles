import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import {
  InstrumentBadge,
  StateBadge,
  ActionBadge,
  PnL,
} from "../../src/Semantics";

it("labels option types independently of whether the option made money", () => {
  render(
    <>
      <InstrumentBadge asset="option" right="call" />
      <PnL value="-120" currency="USD" />
      <InstrumentBadge asset="option" right="put" />
      <PnL value="80" currency="USD" />
    </>,
  );
  expect(screen.getByLabelText("Option type: Call")).toHaveTextContent("Call");
  expect(screen.getByLabelText("Option type: Put")).toHaveTextContent("Put");
  expect(screen.getByLabelText("Loss: -US$120.00")).toBeVisible();
  expect(screen.getByLabelText("Gain: +US$80.00")).toBeVisible();
});

it("distinguishes lifecycle states and transaction actions with explicit labels", () => {
  render(
    <>
      <StateBadge state="Open" />
      <StateBadge state="Partially closed" />
      <StateBadge state="Closed" />
      <StateBadge state="winding_down" />
      <ActionBadge action="open" />
      <ActionBadge action="close" />
      <ActionBadge action="assignment" />
      <ActionBadge action="expiry" />
    </>,
  );
  for (const state of ["Open", "Partially closed", "Closed", "Winding Down"])
    expect(screen.getByLabelText(`Status: ${state}`)).toBeVisible();
  for (const action of ["Open", "Close", "Assignment", "Expiry"])
    expect(
      screen.getByLabelText(`Transaction action: ${action}`),
    ).toBeVisible();
});

it("keeps missing and zero results distinct from gains and losses", () => {
  render(
    <>
      <PnL value={undefined} />
      <PnL value="0.00" currency="USD" />
    </>,
  );
  expect(screen.getByLabelText("Unavailable")).toHaveTextContent("Unavailable");
  expect(screen.getByLabelText("No gain or loss: US$0.00")).toHaveTextContent(
    "US$0.00",
  );
  expect(screen.queryByLabelText(/^Gain:/)).not.toBeInTheDocument();
  expect(screen.queryByLabelText(/^Loss:/)).not.toBeInTheDocument();
});
