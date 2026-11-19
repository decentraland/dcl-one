import { catalystBase, CatalystError } from "../client";

export type DraftOptions = {
  fetch: (url: string, init?: RequestInit) => Promise<Response>;
  base?: string;
  signal?: AbortSignal;
};

export async function draftRequest(path: string, opts: DraftOptions, init: RequestInit = {}): Promise<unknown> {
  const url = `${catalystBase(opts.base)}${path}`;
  const response = await opts.fetch(url, { ...init, signal: opts.signal, cache: "no-store" });
  const body = await response.json().catch(() => null) as { data?: unknown; message?: string; error?: string } | null;
  if (!response.ok) throw new CatalystError(body?.message ?? body?.error ?? `Could not save the draft (${response.status}).`, url, response.status);
  if (!body || !("data" in body)) throw new Error("Builder returned an invalid draft response.");
  return body.data;
}
