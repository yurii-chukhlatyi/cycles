import {
  ArrowDownRight,
  ArrowUpRight,
  Check,
  CircleDot,
  CircleDashed,
  Clock3,
  Minus,
  Plus,
  Repeat2,
} from "lucide-react";
import type { Money } from "./types";
import { label, money, sign } from "./format";

type Tone = ReturnType<typeof sign>;

export function OutcomeText({ value, tone }: { value: string; tone: Tone }) {
  const positive = tone === "positive";
  const negative = tone === "negative";
  const text = positive ? `+${value}` : value;
  const description =
    tone === "muted"
      ? "Unavailable"
      : `${positive ? "Gain" : negative ? "Loss" : "No gain or loss"}: ${text}`;
  return (
    <span className={`outcome-value ${tone}`} aria-label={description}>
      {positive ? (
        <ArrowUpRight size={13} aria-hidden="true" />
      ) : negative ? (
        <ArrowDownRight size={13} aria-hidden="true" />
      ) : null}
      <span>{text}</span>
    </span>
  );
}

export function PnL({
  value,
  currency = "USD",
}: {
  value: Money | null | undefined;
  currency?: string;
}) {
  return <OutcomeText value={money(value, currency)} tone={sign(value)} />;
}

export function InstrumentBadge({
  asset,
  right,
}: {
  asset: string;
  right?: string;
}) {
  const option = asset === "option";
  const name = option
    ? right
      ? label(right)
      : "Unknown option"
    : label(asset);
  return (
    <span
      className={`semantic-badge instrument-badge tone-${option && right ? right : "neutral"}`}
      aria-label={`${option ? "Option type" : "Asset type"}: ${name}`}
    >
      {name}
    </span>
  );
}

export function StateBadge({ state }: { state: string }) {
  const key = state.toLowerCase().replaceAll(" ", "_");
  const name = state.includes("_") ? label(state) : state;
  const closed = key === "closed";
  const partial = key === "partially_closed" || key === "winding_down";
  const open = key === "open" || key === "active";
  const Icon = closed ? Check : partial ? CircleDashed : CircleDot;
  return (
    <span
      className={`semantic-badge state-badge tone-${closed ? "closed" : partial ? "partial" : open ? "open" : "neutral"}`}
      aria-label={`Status: ${name}`}
    >
      <Icon size={11} aria-hidden="true" />
      {name}
    </span>
  );
}

export function ActionBadge({ action }: { action: string }) {
  const tone =
    action === "open"
      ? "call"
      : action === "close"
        ? "put"
        : ["exercise", "assignment"].includes(action)
          ? "partial"
          : "neutral";
  const Icon =
    action === "open"
      ? Plus
      : action === "close"
        ? Minus
        : action === "expiry"
          ? Clock3
          : Repeat2;
  return (
    <span
      className={`semantic-badge action-badge tone-${tone}`}
      aria-label={`Transaction action: ${label(action)}`}
    >
      <Icon size={11} aria-hidden="true" />
      {label(action)}
    </span>
  );
}
