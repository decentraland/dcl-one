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

export function beginShellBrowserSignIn(): ShellSignIn {
  const id = requestId();

  let cancelled = false;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const identity = (async (): Promise<AuthIdentity | null> => {
    const { buildEphemeralMessage, DEFAULT_EXPIRATION_MS, generateEphemeralKey } =
      await import("./identity");
    if (cancelled) return null;
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
    const deadline = Date.now() + POLL_TIMEOUT_MS;
    while (!cancelled && Date.now() < deadline) {
      await new Promise<void>((resolve) => {
        timer = setTimeout(resolve, POLL_INTERVAL_MS);
      });
      if (cancelled) break;
      let res: Response;
      try {
        res = await fetch(`${RELAY_PATH}?id=${encodeURIComponent(id)}`, {
          cache: "no-store",
        });
      } catch {
        continue;
      }
      if (res.status !== 200) continue;
      const body = (await res.json()) as { signer?: string; signature?: string };
      if (!body?.signer || !body?.signature) continue;

      const { recoverMessageAddress } = await import("viem");
      const recovered = await recoverMessageAddress({
        message,
        signature: body.signature as `0x${string}`,
      });
      if (recovered.toLowerCase() !== body.signer.toLowerCase()) {
        continue;
      }

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
  })();

  return {
    identity,
    cancel: () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    },
  };
}
