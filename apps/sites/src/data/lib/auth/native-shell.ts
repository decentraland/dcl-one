import { setIdentity } from "./session";
import type { AuthIdentity, AuthLink } from "./types";
import { forgetSignIn } from "./signin-kind";

export const RELAY_PATH = "/internal/native-auth-relay";
const APPROVE_PATH = "/auth/native";

const POLL_INTERVAL_MS = 2_000;
const POLL_TIMEOUT_MS = 5 * 60_000;

type ShellGlobal = { shell?: string; version?: string };

export function isDesktopShell(): boolean {
  if (typeof window === "undefined") return false;
  const g = (window as { __DCL_SHELL__?: ShellGlobal }).__DCL_SHELL__;
  return g?.shell === "tauri";
}

function requestId(): string {
  const bytes = new Uint8Array(8);
  crypto.getRandomValues(bytes);
  const extra = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  return `${crypto.randomUUID()}-${extra}`;
}

type ShellSignIn = {
  identity: Promise<AuthIdentity | null>;
  cancel: () => void;
};

function waitForNextPoll(signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    function finish() {
      clearTimeout(timer);
      signal.removeEventListener("abort", finish);
      resolve();
    }
    const timer = setTimeout(finish, POLL_INTERVAL_MS);
    signal.addEventListener("abort", finish, { once: true });
  });
}

export function beginShellBrowserSignIn(): ShellSignIn {
  const id = requestId();
  const controller = new AbortController();
  const { signal } = controller;
  const timeout = setTimeout(() => controller.abort(), POLL_TIMEOUT_MS);

  const identity = (async (): Promise<AuthIdentity | null> => {
    const { buildEphemeralMessage, DEFAULT_EXPIRATION_MS, generateEphemeralKey } =
      await import("./identity");
    if (signal.aborted) return null;
    const ephemeral = generateEphemeralKey();
    const expiration = new Date(Date.now() + DEFAULT_EXPIRATION_MS);
    const expirationIso = expiration.toISOString();

    const params = new URLSearchParams({
      id,
      ephemeral: ephemeral.address,
      expiration: expirationIso,
    });
    const approveUrl = `${window.location.origin}${APPROVE_PATH}?${params.toString()}`;

    const w = window as unknown as {
      __DCL_SHELL_LAST_AUTH_URL?: string;
      __DCL_SHELL_TEST_SUPPRESS_NAV__?: boolean;
    };
    w.__DCL_SHELL_LAST_AUTH_URL = approveUrl;
    if (!w.__DCL_SHELL_TEST_SUPPRESS_NAV__) window.location.href = approveUrl;

    const message = buildEphemeralMessage(ephemeral.address, expiration);
    while (!signal.aborted) {
      await waitForNextPoll(signal);
      if (signal.aborted) break;

      let body: { signer?: string; signature?: string };
      try {
        const res = await fetch(`${RELAY_PATH}?id=${encodeURIComponent(id)}`, {
          cache: "no-store",
          signal,
        });
        if (res.status !== 200) {
          // Every poll before the approval answers without the signature: its body is let go, not left open.
          await res.body?.cancel().catch(() => {});
          continue;
        }
        body = await res.json();
      } catch {
        continue;
      }
      if (signal.aborted || typeof body?.signer !== "string" || typeof body?.signature !== "string") continue;

      const { recoverMessageAddress } = await import("viem");
      let recovered: string;
      try {
        recovered = await recoverMessageAddress({
          message,
          signature: body.signature as `0x${string}`,
        });
      } catch {
        continue;
      }
      if (signal.aborted || recovered.toLowerCase() !== body.signer.toLowerCase()) continue;

      const authChain: AuthLink[] = [
        { type: "SIGNER", payload: body.signer.toLowerCase(), signature: "" },
        { type: "ECDSA_EPHEMERAL", payload: message, signature: body.signature },
      ];
      const next: AuthIdentity = {
        signer: body.signer.toLowerCase(),
        ephemeral,
        expiration: expirationIso,
        authChain,
      };
      // Not recorded as a sign-in kind (signin-kind.ts): the approval happened in the system browser, by whatever
      // method the user chose there, so this webview cannot tell, and an earlier record (left by an identity that
      // expired rather than signed out) no longer says how this one signed. It is forgotten, so no Marketplace here.
      forgetSignIn();
      setIdentity(next);
      return next;
    }
    return null;
  })().finally(() => {
    clearTimeout(timeout);
    // Settled either way: nothing else may start or stay pending on this request.
    controller.abort();
  });

  return {
    identity,
    cancel: () => controller.abort(),
  };
}
