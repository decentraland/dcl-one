import { startTransition, StrictMode } from "react";
import { hydrateRoot } from "react-dom/client";
import { HydratedRouter } from "react-router/dom";

import { claimWalletCookieOnUse, resyncWalletCookie } from "@/data/lib/auth/session";
import { installValidationReporter } from "@/data/lib/catalyst/validate-reporter";
import { adoptExplorerHost } from "@/features/components/chrome/explorer-host";

installValidationReporter();
// Framed by the Explorer's Create panel: sign as the Explorer account from the first render.
// Anywhere else the wallet cookie follows this site's own session again, and is taken back each
// time the page is used; a page the server rendered for another wallet (the Explorer's, which its
// Create panel pointed the cookie at) reloads as this session's.
const framed = adoptExplorerHost(window) !== null;
if (!framed) claimWalletCookieOnUse(window);

if (!framed && resyncWalletCookie()) {
  window.location.reload();
} else {
  startTransition(() => {
    hydrateRoot(
      document,
      <StrictMode>
        <HydratedRouter />
      </StrictMode>,
    );
  });
}
