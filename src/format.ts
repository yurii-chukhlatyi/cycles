import type { Money } from "./types";
export function money(x: Money | null | undefined, currency = "USD") {
  if (x == null) return "Unavailable";
  const formatter = new Intl.NumberFormat("en-CA", {
    style: "currency",
    currency,
  });
  const places = formatter.resolvedOptions().maximumFractionDigits ?? 2;
  const raw = String(x);
  const match = /^(-?)(\d+)(?:\.(\d*))?$/.exec(raw);
  if (!match) return formatter.format(Number(x));
  const fraction = match[3] ?? "";
  const factor = 10n ** BigInt(places);
  let units =
    BigInt(match[2]) * factor +
    BigInt((fraction + "0".repeat(places)).slice(0, places) || "0");
  if ((fraction[places] ?? "0") >= "5") units += 1n;
  const integer = new Intl.NumberFormat("en-CA", {
    maximumFractionDigits: 0,
  }).format(units / factor);
  const decimals = (units % factor).toString().padStart(places, "0");
  const negative = match[1] === "-" && units !== 0n;
  return formatter
    .formatToParts(negative ? -1 : 1)
    .map((part) =>
      part.type === "integer"
        ? integer
        : part.type === "fraction"
          ? decimals
          : part.value,
    )
    .join("");
}
export function quantity(x: Money | null | undefined) {
  return x == null
    ? "—"
    : new Intl.NumberFormat("en-CA", { maximumFractionDigits: 8 }).format(
        Number(x),
      );
}
export function percent(x: number | null | undefined) {
  return x == null
    ? "Unavailable"
    : new Intl.NumberFormat("en-CA", {
        style: "percent",
        maximumFractionDigits: 2,
      }).format(x);
}
export const sign = (x: Money | null | undefined) =>
  x == null
    ? "muted"
    : Number(x) < 0
      ? "negative"
      : Number(x) > 0
        ? "positive"
        : "";
export const label = (x: string) =>
  x.replaceAll("_", " ").replace(/\b\w/g, (c) => c.toUpperCase());
