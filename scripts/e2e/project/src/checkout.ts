import { delivery, goods, total, type Line } from "./cart";
import { find } from "./products";

export type Receipt = { rows: string[]; goods: string; delivery: string; total: string };

/** A price as the shop writes it: whole dollars bare, cents when there are any. */
export function price(amount: number): string {
  return Number.isInteger(amount) ? `$${amount}` : `$${amount.toFixed(2)}`;
}

/** What the last page shows before paying: a row a line, then the sums. */
export function receipt(lines: Line[]): Receipt {
  return {
    rows: lines.map((line) => `${line.count} × ${find(line.id)?.name ?? line.id}  ${price(line.price * line.count)}`),
    goods: price(goods(lines)),
    delivery: delivery(lines) === 0 ? "free" : price(delivery(lines)),
    total: price(total(lines)),
  };
}
