import type { OverlayPush } from "../../generated/bridge/OverlayPush";
import type { SignedFetchPayload } from "../../generated/bridge/SignedFetchPayload";
import { CatalystError, forwardFailure, requestUrl, type SendOpts } from "@web-core/catalyst/client";

export * from "@web-core/catalyst/client";

type SignedFetchOpts = { method?: string; body?: unknown; timeoutMs?: number };

type SignedFetchResult = { status: number; body: string };

type SignedFetchPush = Partial<
  Extract<OverlayPush, { kind: "signedFetchResult" }>
>;

function isSignedFetchPush(v: unknown): v is SignedFetchPush {
  return !!v && typeof v === "object";
}

export function signedFetch(url: string, opts: SignedFetchOpts = {}): Promise<SignedFetchResult> {
  return new Promise<SignedFetchResult>((resolve, reject) => {
    const bridge = typeof window !== "undefined" ? window.dclBridge : undefined;
    const send = bridge?.send;
    const onState = bridge?.onState;
    if (!bridge || typeof send !== "function" || typeof onState !== "function") {
      reject(
        new CatalystError("Signed fetch unavailable: engine bridge not ready", url),
      );
      return;
    }

    const id =
      typeof crypto !== "undefined" && crypto.randomUUID
        ? crypto.randomUUID()
        : `sf-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const method = (opts.method ?? "POST").toUpperCase();
    const hasBody = opts.body !== undefined && opts.body !== null;
    const body = hasBody
      ? typeof opts.body === "string"
        ? opts.body
        : JSON.stringify(opts.body)
      : undefined;

    let unsubscribe: () => void = () => {};
    let timer: ReturnType<typeof setTimeout> | null = null;
    const settle = <A>(fn: (arg: A) => void, arg: A) => {
      try {
        unsubscribe();
      } catch {
      }
      if (timer) clearTimeout(timer);
      fn(arg);
    };

    unsubscribe = onState((push) => {
      if (
        isSignedFetchPush(push) &&
        push.kind === "signedFetchResult" &&
        push.id === id
      ) {
        settle(resolve, { status: push.status ?? 0, body: push.body ?? "" });
      }
    });

    const timeoutMs = opts.timeoutMs ?? 30000;
    timer = setTimeout(() => {
      settle(reject, new CatalystError("Signed fetch timed out", url));
    }, timeoutMs);

    const payload: SignedFetchPayload = { id, url, method, body };
    send("SignedFetch", payload);
  });
}

export async function sendSignedJSON<T = unknown>(
  path: string,
  opts: SendOpts = {},
): Promise<T | null> {
  const url = requestUrl(path, opts);

  const { status, body } = await signedFetch(url, {
    method: opts.method ?? "POST",
    body: opts.body,
    timeoutMs: opts.timeoutMs,
  });

  if (status < 200 || status >= 300) throw forwardFailure(url, status, body);

  if (status === 204 || !body) return null;
  try {
    return JSON.parse(body) as T;
  } catch {
    return body as unknown as T;
  }
}
