// from: v2 ui3/src/marketplace/native-context.ts (moved here; the old path is removed)
// Whether the Shop is mounted natively inside the Explorer (the native contract's runtime), where a
// page reload would drop the running engine and earning/claiming controls are omitted.

import { createContext, useContext } from "react";

export const NativeShopContext = createContext(false);
export const useNativeShop = () => useContext(NativeShopContext);

/** The names the marketplace runtime and sites already import. */
export const NativeMarketplaceContext = NativeShopContext;
export const useNativeMarketplace = useNativeShop;
