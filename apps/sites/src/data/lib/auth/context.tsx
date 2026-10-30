import { useCallback, useMemo, useState, useSyncExternalStore } from "react";

import { isDevHost } from "./dev-host";
import { isIdentityExpired } from "./expiry";
import {
  clearIdentity as clearStored,
  getIdentity,
  getServerSnapshot,
  hostOwnsSession,
  setIdentity,
  subscribe,
} from "./session";
import type { SignedRequest } from "./signer";
import {
  clearThirdwebSession,
  setThirdwebSession,
} from "./thirdweb/session";
import { hasThirdwebClientId } from "./thirdweb/config";
import type { ThirdwebAuthResult, ThirdwebSocialProvider } from "./thirdweb/api";
import type { WalletConnectSession } from "./walletconnect";
import type { AuthIdentity, SignedFetchMetadata } from "./types";
import { detectWallets, hasWallet, selectWallet, type DetectedWallet } from "./wallet";

async function identityFromInApp(
  auth: Pick<ThirdwebAuthResult, "token" | "walletAddress">,
  opts?: { expirationMs?: number },
): Promise<AuthIdentity> {
  const [{ createIdentityWith }, { makeInAppSigner }] = await Promise.all([
    import("./identity"),
    import("./thirdweb/signer"),
  ]);
  const signer = makeInAppSigner(auth);
  const next = await createIdentityWith(signer.address, signer.personalSign, opts);
  setThirdwebSession({ token: signer.token, address: signer.address });
  return next;
}

type AuthStatus = "anonymous" | "connecting" | "connected" | "expired";

type UseAuth = {
  identity: AuthIdentity | null;
  address: string | null;
  status: AuthStatus;
  isConnected: boolean;
  walletAvailable: boolean;
  detectedWallets: DetectedWallet[];
  inAppAvailable: boolean;
  error: string | null;
  connect: (opts?: {
    expirationMs?: number;
    walletRdns?: string;
  }) => Promise<AuthIdentity | null>;
  connectWalletConnect: (opts?: {
    signal?: AbortSignal;
    expirationMs?: number;
  }) => Promise<WalletConnectSession>;
  startEmailSignIn: (email: string) => Promise<void>;
  verifyEmailSignIn: (
    email: string,
    code: string,
    opts?: { expirationMs?: number },
  ) => Promise<AuthIdentity | null>;
  startSocialSignIn: (
    provider: ThirdwebSocialProvider,
    redirectUrl: string,
  ) => void;
  completeInAppSession: (
    auth: Pick<ThirdwebAuthResult, "token" | "walletAddress">,
    opts?: { expirationMs?: number },
  ) => Promise<AuthIdentity | null>;
  devHost: boolean;
  connectWithKey: (
    signerPrivateKey?: `0x${string}`,
    opts?: { expirationMs?: number },
  ) => Promise<AuthIdentity | null>;
  disconnect: () => void;
  sign: (
    method: string,
    url: string,
    metadata?: SignedFetchMetadata,
  ) => Promise<SignedRequest>;
  fetch: (
    url: string,
    init?: RequestInit & { metadata?: SignedFetchMetadata },
  ) => Promise<Response>;
};

export function useAuth(): UseAuth {
  const identity = useSyncExternalStore(
    subscribe,
    getIdentity,
    getServerSnapshot,
  );
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const expired = !!identity && isIdentityExpired(identity);

  const connect = useCallback(
    async (opts?: { expirationMs?: number; walletRdns?: string }) => {
      setError(null);
      setConnecting(true);
      try {
        if (opts?.walletRdns) selectWallet(opts.walletRdns);
        const { createIdentity } = await import("./identity");
        const next = await createIdentity(opts);
        setIdentity(next);
        return next;
      } catch (err) {
        setError((err as Error)?.message ?? "Failed to connect wallet.");
        return null;
      } finally {
        setConnecting(false);
      }
    },
    [],
  );

  const connectWalletConnect = useCallback(
    async (opts?: { signal?: AbortSignal; expirationMs?: number }) => {
      setError(null);
      let session: WalletConnectSession;
      try {
        const { connectWalletConnect } = await import("./walletconnect");
        session = await connectWalletConnect(opts);
      } catch (err) {
        setError((err as Error)?.message ?? "Couldn't start phone sign-in.");
        throw err;
      }
      return session;
    },
    [],
  );

  const startEmailSignIn = useCallback(async (email: string) => {
    setError(null);
    try {
      const { initiateEmailLogin } = await import("./thirdweb/api");
      await initiateEmailLogin(email);
    } catch (err) {
      const msg = (err as Error)?.message ?? "Couldn't send the sign-in code.";
      setError(msg);
      throw err;
    }
  }, []);

  const completeInAppSession = useCallback(
    async (
      auth: Pick<ThirdwebAuthResult, "token" | "walletAddress">,
      opts?: { expirationMs?: number },
    ) => {
      setError(null);
      setConnecting(true);
      try {
        const next = await identityFromInApp(auth, opts);
        setIdentity(next);
        return next;
      } catch (err) {
        setError((err as Error)?.message ?? "Failed to finish sign-in.");
        return null;
      } finally {
        setConnecting(false);
      }
    },
    [],
  );

  const verifyEmailSignIn = useCallback(
    async (email: string, code: string, opts?: { expirationMs?: number }) => {
      setError(null);
      setConnecting(true);
      try {
        const { completeEmailLogin } = await import("./thirdweb/api");
        const result = await completeEmailLogin(email, code);
        const next = await identityFromInApp(result, opts);
        setIdentity(next);
        return next;
      } catch (err) {
        setError((err as Error)?.message ?? "That code didn't work.");
        return null;
      } finally {
        setConnecting(false);
      }
    },
    [],
  );

  const startSocialSignIn = useCallback(
    async (provider: ThirdwebSocialProvider, redirectUrl: string) => {
      setError(null);
      if (typeof window !== "undefined") {
        try {
          const { socialLoginUrl } = await import("./thirdweb/api");
          window.location.href = socialLoginUrl(provider, redirectUrl);
        } catch (err) {
          setError((err as Error)?.message ?? "Couldn't start social sign-in.");
        }
      }
    },
    [],
  );

  const connectWithKey = useCallback(
    async (signerPrivateKey?: `0x${string}`, opts?: { expirationMs?: number }) => {
      setError(null);
      setConnecting(true);
      try {
        const { signInWithPrivateKey } = await import("./dev-identity");
        return await signInWithPrivateKey(signerPrivateKey, opts);
      } catch (err) {
        setError((err as Error)?.message ?? "Failed to sign in with key.");
        return null;
      } finally {
        setConnecting(false);
      }
    },
    [],
  );

  const disconnect = useCallback(() => {
    setError(null);
    // The Explorer's account is the Explorer's to end; the website's own session stays as it is.
    if (hostOwnsSession()) return;
    clearThirdwebSession();
    clearStored();
    void fetch("/auth/callback", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ signout: true }),
      keepalive: true,
    }).catch(() => {});
  }, []);

  const sign = useCallback(
    async (method: string, url: string, metadata?: SignedFetchMetadata) => {
      const id = getIdentity();
      if (!id) throw new Error("Not connected.");
      const { signRequest } = await import("./signer");
      return signRequest(id, method, url, metadata ?? {});
    },
    [],
  );

  const boundFetch = useCallback(
    async (url: string, init?: RequestInit & { metadata?: SignedFetchMetadata }) => {
      const id = getIdentity();
      if (!id) throw new Error("Not connected.");
      const { signedFetch } = await import("./signer");
      return signedFetch(id, url, init);
    },
    [],
  );

  const status: AuthStatus = connecting
    ? "connecting"
    : expired
      ? "expired"
      : identity
        ? "connected"
        : "anonymous";

  return useMemo(
    () => ({
      identity: expired ? null : identity,
      address: expired ? null : (identity?.signer ?? null),
      status,
      isConnected: !!identity && !expired,
      walletAvailable: hasWallet(),
      detectedWallets: detectWallets(),
      inAppAvailable: hasThirdwebClientId(),
      error,
      connect,
      connectWalletConnect,
      startEmailSignIn,
      verifyEmailSignIn,
      startSocialSignIn,
      completeInAppSession,
      devHost: isDevHost(),
      connectWithKey,
      disconnect,
      sign,
      fetch: boundFetch,
    }),
    [
      identity,
      expired,
      status,
      error,
      connect,
      connectWalletConnect,
      startEmailSignIn,
      verifyEmailSignIn,
      startSocialSignIn,
      completeInAppSession,
      connectWithKey,
      disconnect,
      sign,
      boundFetch,
    ],
  );
}
