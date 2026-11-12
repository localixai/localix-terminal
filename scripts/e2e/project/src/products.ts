export type Product = { id: string; name: string; notes: string; price: number; kind: "coffee" | "tea" | "gear" };

/** What the shop sells, in the order the front page shows it. */
export const PRODUCTS: Product[] = [
  { id: "guji", name: "Ethiopia Guji", notes: "Peach, jasmine, black tea", price: 18, kind: "coffee" },
  { id: "huila", name: "Colombia Huila", notes: "Cocoa, red apple, caramel", price: 16, kind: "coffee" },
  { id: "nyeri", name: "Kenya Nyeri", notes: "Blackcurrant, grapefruit", price: 19, kind: "coffee" },
];

/** The product with this id, if the shop sells it. */
export function find(id: string): Product | undefined {
  return PRODUCTS.find((product) => product.id === id);
}

/** The products of one shelf: what a link of the header shows. */
export function shelf(kind: Product["kind"]): Product[] {
  return PRODUCTS.filter((product) => product.kind === kind);
}
