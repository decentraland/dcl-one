import { mountShop } from "../shop/native.client";

// The Explorer's first native entry, /marketplace-native: an alias of /shop-native (the same runtime
// and handle contract) for Explorer builds that still fetch the old name.
export const handle = { version: 1 as const, mount: mountShop };
export default function MarketplaceNativeEntry() {
  return null;
}
