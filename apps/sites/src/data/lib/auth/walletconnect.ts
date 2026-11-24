import { startWalletConnectSignIn } from "@web-core/auth/walletconnect";
import { setIdentity } from "./session";
import { recordSignIn } from "./signin-kind";
export { hasWalletConnect } from "@web-core/auth/walletconnect";
export type { WalletConnectSession } from "@web-core/auth/walletconnect";

export async function connectWalletConnect(opts: { signal?: AbortSignal; expirationMs?: number } = {}) {
  const session = await startWalletConnectSignIn(opts);
  return {
    ...session,
    connected: session.connected.then((identity) => {
      if (opts.signal?.aborted) return null;
      if (identity) {
        recordSignIn(identity.signer, "web3");
        setIdentity(identity);
      }
      return identity;
    }),
  };
}
