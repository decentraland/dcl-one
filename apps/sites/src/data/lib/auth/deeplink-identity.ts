import { privateKeyToAccount } from "viem/accounts";

import { toStoredIdentity, type StoredAuthIdentity } from "@ui/data/auth/identity";

import { signedFetch } from "./signer";
import type { AuthIdentity } from "./types";

export const DEEPLINK_IDENTITY_EXPIRATION_MS = 30 * 24 * 60 * 60 * 1000;
export const AUTH_API_CLIENT_TAG = "dcl-web/1";

export function authCorrelationId(): string {
  try {
    return crypto.randomUUID();
  } catch {
    return `web-${Date.now().toString(36)}`;
  }
}

export function authClientHeaders(values?: HeadersInit, correlationId = authCorrelationId()): Headers {
  const headers = new Headers(values);
  headers.set("X-DCL-Client", AUTH_API_CLIENT_TAG);
  headers.set("X-Correlation-ID", correlationId);
  return headers;
}

export type HandoffIdentity = StoredAuthIdentity & {
  ephemeralIdentity: { address: string; privateKey: string; publicKey: string };
};

export function toHandoffIdentity(identity: AuthIdentity): HandoffIdentity {
  const stored = toStoredIdentity(identity);
  return {
    ...stored,
    ephemeralIdentity: {
      ...stored.ephemeralIdentity,
      publicKey: privateKeyToAccount(identity.ephemeral.privateKey).publicKey,
    },
  };
}

export type IdentityHandoffResponse = { identityId: string; expiration: string };

export async function postIdentityHandoff(
  identity: AuthIdentity,
  authApiUrl: string,
  opts: { isMobile?: boolean; signal?: AbortSignal } = {},
): Promise<IdentityHandoffResponse> {
  const res = await signedFetch(identity, `${authApiUrl}/identities`, {
    method: "POST",
    headers: authClientHeaders({ "content-type": "application/json" }),
    body: JSON.stringify({
      identity: toHandoffIdentity(identity),
      isMobile: opts.isMobile ?? false,
    }),
    signal: opts.signal,
  });
  const body = (await res.json().catch(() => null)) as
    | { identityId?: unknown; expiration?: unknown; error?: unknown }
    | null;
  if (!res.ok) {
    throw new Error(
      typeof body?.error === "string" && body.error ? body.error : "Failed to create identity",
    );
  }
  if (typeof body?.identityId !== "string" || !body.identityId) {
    throw new Error("The sign-in server returned no identity id.");
  }
  return {
    identityId: body.identityId,
    expiration: typeof body.expiration === "string" ? body.expiration : "",
  };
}
