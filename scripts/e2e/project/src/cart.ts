import type { Product } from "./products";

export type Line = { id: string; price: number; count: number };

/** Delivery costs $4 on a cart under $30, and nothing from there. */
const DELIVERY = 4;
const FREE_FROM = 30;

/** What the goods cost, before delivery. */
export function goods(lines: Line[]): number {
  return lines.reduce((sum, line) => sum + line.price * line.count, 0);
}

/** What delivery costs for this cart: nothing for an empty one. */
export function delivery(lines: Line[]): number {
  return lines.length === 0 || goods(lines) >= FREE_FROM ? 0 : DELIVERY;
}

/** What a cart costs, delivery included: free from $30. */
export function total(lines: Line[]): number {
  return goods(lines) + delivery(lines);
}

/** How many things are in the cart: what the header's button counts. */
export function count(lines: Line[]): number {
  return lines.reduce((sum, line) => sum + line.count, 0);
}

/** A cart with one more of a product: a line of its own the first time, one more on it after. */
export function add(lines: Line[], product: Product): Line[] {
  if (!lines.some((line) => line.id === product.id)) {
    return [...lines, { id: product.id, price: product.price, count: 1 }];
  }
  return lines.map((line) => (line.id === product.id ? { ...line, count: line.count + 1 } : line));
}

/** A cart with one fewer of a product: its line goes with the last one. */
export function remove(lines: Line[], id: string): Line[] {
  return lines
    .map((line) => (line.id === id ? { ...line, count: line.count - 1 } : line))
    .filter((line) => line.count > 0);
}
