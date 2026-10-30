import { mountShop } from "../shop/native.client";

// The Explorer's native Shop runtime (ui3/src/shop/native-contract.ts, version 1), built with the same
// route splitting as the website; entry.server publishes its hashed module URL at /shop-native.
export const handle = { version: 1 as const, mount: mountShop };
export default function ShopNativeEntry() {
  return null;
}
