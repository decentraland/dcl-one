import { z } from "zod";

import { getJSON } from "./client";

const About = z.object({ configurations: z.object({ networkId: z.number() }) });

const READ_TIMEOUT_MS = 3000;
const FAILURE_TTL_MS = 10_000;

let known: number | undefined;
let failedAt = 0;

/**
 * The Ethereum network the node works on, as its `/about` answers it (`networkId`: 1 mainnet, 11155111
 * sepolia). A node never changes network, so the answer is kept; undefined while the node can't tell.
 */
export async function nodeNetworkId(): Promise<number | undefined> {
  if (known !== undefined || Date.now() - failedAt < FAILURE_TTL_MS) return known;
  try {
    const about = await getJSON("/about", { signal: AbortSignal.timeout(READ_TIMEOUT_MS) });
    known = About.parse(about).configurations.networkId;
  } catch {
    failedAt = Date.now();
  }
  return known;
}
