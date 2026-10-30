import type { Hex } from "viem";
import { createIdentityWith, type AuthIdentity } from "./identity";

export type WalletConnectSession<T = AuthIdentity> = {
  uri: string;
  qrDataUrl: string;
  connected: Promise<T | null>;
  cancel: () => void;
};

export function hasWalletConnect(): boolean {
  return walletConnectProjectId().length > 0;
}

/** The explorer is served as a static document, outside the sites root loader. */
export async function loadWalletConnectConfig(): Promise<boolean> {
  if (hasWalletConnect()) return true;
  const response = await fetch("/internal/auth-config");
  if (!response.ok) throw new Error("Couldn't load wallet sign-in configuration.");
  const config: unknown = await response.json();
  if (!config || typeof config !== "object" || !("walletConnectProjectId" in config)
    || typeof config.walletConnectProjectId !== "string") {
    throw new Error("Invalid wallet sign-in configuration.");
  }
  window.__DCL_PUBLIC__ = { ...window.__DCL_PUBLIC__, walletConnectProjectId: config.walletConnectProjectId };
  return hasWalletConnect();
}

function walletConnectProjectId(): string {
  return (typeof window !== "undefined" && window.__DCL_PUBLIC__?.walletConnectProjectId)
    || import.meta.env?.VITE_DCL_WALLETCONNECT_PROJECT_ID || "";
}

let clientPromise: Promise<import("@walletconnect/sign-client").default> | undefined;
async function signingClient(projectId: string) {
  if (!clientPromise) {
    clientPromise = import("@walletconnect/sign-client").then(({ default: SignClient }) =>
      SignClient.init({
        projectId,
        metadata: {
          name: "Decentraland",
          description: "Sign in to Decentraland",
          url: window.location.origin,
          icons: [],
        },
      }),
    ).catch((error) => { clientPromise = undefined; throw error; });
  }
  return clientPromise;
}

/** Authorize the existing Decentraland session key with a standard wallet.
 * WalletConnect carries one signature; the delegation has its own lifetime.
 */
export async function startWalletConnect<T>(
  approve: (address: string, sign: (message: string) => Promise<string>) => Promise<T>,
  opts: { signal?: AbortSignal } = {},
): Promise<WalletConnectSession<T>> {
  const projectId = walletConnectProjectId();
  if (!projectId) throw new Error("WalletConnect is not configured for this site.");
  opts.signal?.throwIfAborted();
  const client = await signingClient(projectId);
  opts.signal?.throwIfAborted();
  const { uri, approval } = await client.connect({
    requiredNamespaces: {
      eip155: { chains: ["eip155:1"], methods: ["personal_sign"], events: [] },
    },
  });
  const controller = new AbortController();
  const cancel = () => controller.abort();
  opts.signal?.addEventListener("abort", cancel, { once: true });
  const timeout = setTimeout(cancel, 5 * 60_000);
  const cancelled = new Promise<null>((resolve) => {
    controller.signal.addEventListener("abort", () => resolve(null), { once: true });
  });
  const work = (async () => {
    const session = await approval();
    try {
      if (controller.signal.aborted) return null;
      const account = session.namespaces.eip155?.accounts[0]?.split(":");
      const address = account?.[2];
      const { isAddress, recoverMessageAddress, stringToHex } = await import("viem");
      if (account?.[0] !== "eip155" || account[1] !== "1" || !address || !isAddress(address)) {
        throw new Error("Wallet returned no valid Ethereum account.");
      }
      controller.signal.throwIfAborted();
      return await approve(address.toLowerCase(), async (message) => {
        controller.signal.throwIfAborted();
        const signature = await client.request<Hex>({
          topic: session.topic,
          chainId: "eip155:1",
          request: { method: "personal_sign", params: [stringToHex(message), address] },
        });
        controller.signal.throwIfAborted();
        const recovered = await recoverMessageAddress({ message, signature });
        if (recovered.toLowerCase() !== address.toLowerCase()) {
          throw new Error("Wallet signature does not match the connected account.");
        }
        return signature;
      });
    } finally {
      // Includes late wallet approval after the user has canceled.
      await client.disconnect({ topic: session.topic, reason: { code: 6000, message: "Sign-in finished" } });
    }
  })();
  const connected = Promise.race([work, cancelled]).finally(() => {
    clearTimeout(timeout);
    opts.signal?.removeEventListener("abort", cancel);
  });
  // The caller receives this promise after QR rendering; observe early failures too.
  void connected.catch(() => {});
  if (opts.signal?.aborted) cancel();
  try {
    if (!uri) throw new Error("WalletConnect did not return a pairing code.");
    const { default: qrcode } = await import("qrcode-generator");
    const qr = qrcode(0, "M");
    qr.addData(uri);
    qr.make();
    controller.signal.throwIfAborted();
    return {
      uri,
      qrDataUrl: "data:image/svg+xml;utf8," + encodeURIComponent(qr.createSvgTag({ cellSize: 6, margin: 2, scalable: true })),
      connected,
      cancel,
    };
  } catch (error) {
    cancel();
    throw error;
  }
}

export function startWalletConnectSignIn(opts: { signal?: AbortSignal; expirationMs?: number } = {}) {
  return startWalletConnect((address, sign) => createIdentityWith(address, sign, opts), opts);
}
