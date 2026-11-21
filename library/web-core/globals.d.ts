// The host globals the library reads beyond ES and the DOM, each as narrow as its use. Only
// library/web-core/tsconfig.json includes this file; the hosts compile the modules with their own
// declarations of the same globals.

interface Window {
  // The page's public configuration, written by the host's document (Sites' root loader).
  __DCL_PUBLIC__?: { walletConnectProjectId?: string; docsBase?: string };
}

// Vite's build-time environment, when the host bundles with Vite.
interface ImportMeta {
  readonly env?: {
    readonly DEV?: boolean;
    readonly VITE_DCL_WALLETCONNECT_PROJECT_ID?: string;
    readonly VITE_SHOP_API_MODE?: string;
    readonly VITE_SHOP_CHAIN_ID?: string;
    readonly VITE_SHOP_ETHEREUM_CHAIN_ID?: string;
    readonly VITE_SITE_URL?: string;
  };
}
