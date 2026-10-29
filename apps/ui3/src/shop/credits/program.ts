// from: marketplace@f3956826:webapp/src/modules/sagas.ts (the dapps CreditsClient: fetchCredits, fetchSeason,
//       createSSEConnection); v2 ui3/src/marketplace/pages/MkCreditsHub.tsx (the program progress panel).
//       Marketplace UI: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.
// The earned-credits program (seasons, weekly goals, progress) and the balance stream. The stream is a
// signed GET that stays open (server-sent events), which an EventSource cannot sign; it is read with
// fetch and a signer the host provides, reconnecting a few times before giving up (the caller then
// falls back to polling).

import type { CreditsProgramProgressResponse } from "@catalyst-api/credits/CreditsProgramProgressResponse";
import type { SeasonsData } from "@catalyst-api/credits/SeasonsData";
import { shopClient, type ShopClient } from "../api/shop";
import type { ShopSigner } from "../api/signer";
import { decodeUserCredits, type UserCreditsResponse } from "./credits";

/** GET /shop-api/v1/credit-program/season: the current season (null when the program is off). */
export async function fetchSeason(client: ShopClient = shopClient()): Promise<unknown | null> {
  try {
    return await client.credits.season<unknown>();
  } catch {
    return null;
  }
}

/** GET /shop-api/v1/credit-program/seasons: last, current and next season. */
export async function fetchSeasons(client: ShopClient = shopClient()): Promise<SeasonsData | null> {
  try {
    return await client.credits.seasons();
  } catch {
    return null;
  }
}

/** GET /shop-api/v1/users/{address}/progress: the account's goals and earned credits (signed). */
export async function fetchProgress(address: string, client: ShopClient = shopClient()): Promise<CreditsProgramProgressResponse | null> {
  try {
    return await client.credits.progress(address);
  } catch {
    return null;
  }
}

export type StreamHandle = { close(): void };

/** Parse `data:` lines of a server-sent-events chunk stream, one JSON message per event. */
export function parseSseChunk(buffer: string): { events: string[]; rest: string } {
  const parts = buffer.split(/\r?\n\r?\n/);
  const rest = parts.pop() ?? "";
  const events = parts
    .map((block) =>
      block
        .split(/\r?\n/)
        .filter((l) => l.startsWith("data:"))
        .map((l) => l.slice(5).trimStart())
        .join("\n"),
    )
    .filter((d) => d.length > 0);
  return { events, rest };
}

/**
 * Follow `GET /shop-api/v1/users/{address}/credits/stream`. `onBalance` gets each decoded update;
 * `onGiveUp` fires after the retries, and the caller keeps polling instead.
 */
export function openCreditsStream(opts: {
  address: string;
  signer: ShopSigner;
  client?: ShopClient;
  fetchImpl?: typeof fetch;
  onBalance: (credits: UserCreditsResponse) => void;
  onGiveUp?: (error: unknown) => void;
  maxRetries?: number;
  retryMs?: number;
}): StreamHandle {
  const client = opts.client ?? shopClient();
  const fetchImpl = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
  const controller = new AbortController();
  let retries = 0;
  let closed = false;
  const run = async (): Promise<void> => {
    const url = client.credits.streamUrl(opts.address);
    const headers = await opts.signer.sign("GET", new URL(url, "http://shop.invalid").pathname, {});
    const res = await fetchImpl(url, { headers: { accept: "text/event-stream", ...headers }, signal: controller.signal });
    if (!res.ok || !res.body) throw new Error(`credits stream answered ${res.status}`);
    retries = 0;
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    for (;;) {
      const { value, done } = await reader.read();
      if (done) throw new Error("credits stream closed");
      buffer += decoder.decode(value, { stream: true });
      const { events, rest } = parseSseChunk(buffer);
      buffer = rest;
      for (const data of events) {
        try {
          opts.onBalance(decodeUserCredits(JSON.parse(data)));
        } catch {
          // A malformed event is skipped, not fatal.
        }
      }
    }
  };
  const loop = async () => {
    while (!closed) {
      try {
        await run();
      } catch (e) {
        if (closed || (e as { name?: string })?.name === "AbortError") return;
        retries += 1;
        if (retries > (opts.maxRetries ?? 5)) {
          opts.onGiveUp?.(e);
          return;
        }
        await new Promise((r) => setTimeout(r, opts.retryMs ?? 3_000));
      }
    }
  };
  void loop();
  return {
    close() {
      closed = true;
      controller.abort();
    },
  };
}
