import type { ShopClient } from "./shop";

let client: ShopClient | null = null;

/** The browser document's shared client, initialized by the first host that reads it. */
export function getDocumentShopClient(create: () => ShopClient): ShopClient {
  return (client ??= create());
}

export function setShopClient(next: ShopClient | null): void {
  client = next;
}
