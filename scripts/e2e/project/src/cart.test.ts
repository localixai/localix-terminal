import { expect, test } from "bun:test";
import { add, count, remove, total, type Line } from "./cart";
import { receipt } from "./checkout";
import { PRODUCTS } from "./products";

const [guji, huila] = PRODUCTS;

test("delivery is free from $30", () => {
  expect(total([{ id: "guji", price: 18, count: 2 }])).toBe(36);
});

test("delivery is $4 under $30", () => {
  expect(total([{ id: "huila", price: 16, count: 1 }])).toBe(20);
});

test("an empty cart costs nothing", () => {
  expect(total([])).toBe(0);
});

test("a product added twice is one line of two", () => {
  const lines = add(add([], guji), guji);
  expect(lines).toEqual([{ id: "guji", price: 18, count: 2 }]);
  expect(count(add(lines, huila))).toBe(3);
});

test("the last one removed takes its line with it", () => {
  expect(remove(add([], guji), "guji")).toEqual([]);
});

test("the receipt names what is bought and says delivery is free", () => {
  const lines: Line[] = [{ id: "guji", price: 18, count: 2 }];
  expect(receipt(lines)).toEqual({ rows: ["2 × Ethiopia Guji  $36"], goods: "$36", delivery: "free", total: "$36" });
});
