import { parseRecoverResponse, type LoadResult, type OutcomeError } from "./auth-request-recovery";
import { authClientHeaders, authCorrelationId } from "@/data/lib/auth/deeplink-identity";

const SAME_ORIGIN_AUTH_API = "/auth-api";
export const AUTH_API_TIMEOUT_MS = 10_000;
const AUTH_API_UNREACHABLE = "Couldn't reach the sign-in server.";

function authApiDeadline(): AbortSignal | undefined {
  return typeof AbortSignal !== "undefined" && typeof AbortSignal.timeout === "function"
    ? AbortSignal.timeout(AUTH_API_TIMEOUT_MS)
    : undefined;
}

export function withDeadline<T>(promise: Promise<T>, ms: number, onTimeout: T): Promise<T> {
  return new Promise<T>((resolve) => {
    const timer = setTimeout(() => resolve(onTimeout), ms);
    const settle = (value: T) => {
      clearTimeout(timer);
      resolve(value);
    };
    promise.then(settle, () => settle(onTimeout));
  });
}

export async function handoffWithDeadline<T>(
  post: (opts: { signal?: AbortSignal }) => Promise<T>,
): Promise<T> {
  try {
    return await post({ signal: authApiDeadline() });
  } catch (err) {
    const name = (err as Error)?.name;
    if (name === "TimeoutError" || name === "AbortError") throw new Error(AUTH_API_UNREACHABLE);
    throw err;
  }
}

type OutcomeBody = { sender: string; result?: unknown; error?: OutcomeError };

export function authApiClient(authApiUrl: string) {
  const correlationId = authCorrelationId();
  const headers = (values?: HeadersInit): Headers => authClientHeaders(values, correlationId);
  const requestUrl = (id: string, suffix = "") =>
    `${authApiUrl.replace(/\/+$/, "")}/v2/requests/${encodeURIComponent(id)}${suffix}`;

  async function loadRequest(id: string): Promise<LoadResult> {
    let res: Response;
    try {
      res = await fetch(requestUrl(id), {
        headers: headers({ accept: "application/json" }),
        cache: "no-store",
        signal: authApiDeadline(),
      });
    } catch {
      return { kind: "error", message: AUTH_API_UNREACHABLE };
    }
    if (res.ok) {
      const request = parseRecoverResponse(await res.json().catch(() => null));
      return request
        ? { kind: "ok", request }
        : { kind: "error", message: "The sign-in server returned an unexpected response." };
    }
    const body = (await res.json().catch(() => null)) as { error?: string } | null;
    const err = body?.error ?? "";
    if (/already been fulfilled|already has a response/.test(err)) return { kind: "fulfilled" };
    if (/has expired/.test(err)) return { kind: "expired" };
    if (/not found/.test(err)) return { kind: "not_found" };
    return { kind: "error", message: err || `The request couldn't be loaded (${res.status}).` };
  }

  async function validationRequirement(id: string): Promise<boolean | null> {
    try {
      const res = await fetch(requestUrl(id, "/validation"), {
        headers: headers(),
        cache: "no-store",
        signal: authApiDeadline(),
      });
      if (!res.ok) return null;
      const body = (await res.json()) as { requiresValidation?: boolean };
      return body?.requiresValidation === true;
    } catch {
      return null;
    }
  }

  async function requiresValidation(id: string): Promise<boolean> {
    return (await validationRequirement(id)) !== false;
  }

  async function postOutcome(id: string, body: OutcomeBody): Promise<Response> {
    return fetch(requestUrl(id, "/outcome"), {
      method: "POST",
      headers: headers({ "content-type": "application/json" }),
      body: JSON.stringify(body),
    });
  }

  async function reportOutcome(id: string, body: OutcomeBody): Promise<null> {
    return withDeadline(
      postOutcome(id, body).then(
        () => null,
        () => null,
      ),
      AUTH_API_TIMEOUT_MS,
      null,
    );
  }

  return { loadRequest, validationRequirement, requiresValidation, postOutcome, reportOutcome };
}

export const { loadRequest, validationRequirement, requiresValidation, postOutcome, reportOutcome } =
  authApiClient(SAME_ORIGIN_AUTH_API);
