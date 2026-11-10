import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { HelpTip } from "../../src/HelpTip";

it("shows help on hover and dismisses it after leaving", async () => {
  const user = userEvent.setup();
  render(<HelpTip label="Example value" text="How this value works." />);
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  const trigger = screen.getByRole("button", { name: "Explain Example value" });
  await user.hover(trigger);
  expect(screen.getByRole("tooltip")).toHaveTextContent(
    "How this value works.",
  );
  expect(trigger).toHaveAccessibleDescription("How this value works.");
  await user.unhover(trigger);
  await waitFor(() =>
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument(),
  );
});

it("supports keyboard focus, Escape, click and outside dismissal", async () => {
  const user = userEvent.setup();
  render(
    <>
      <HelpTip label="Example value" text="How this value works." />
      <button>Outside</button>
    </>,
  );
  const trigger = screen.getByRole("button", { name: "Explain Example value" });
  await user.tab();
  expect(trigger).toHaveFocus();
  expect(screen.getByRole("tooltip")).toBeVisible();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
  fireEvent.click(trigger);
  expect(screen.getByRole("tooltip")).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Outside" }));
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
});
