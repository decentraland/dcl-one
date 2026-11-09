// The browser host's NAME reads: the node's base and fetch for the banned-names list. What is read, and how its
// answer is understood, is names/api.ts.

import { catalystBase } from "../../data/catalyst/client";
import { fetchBannedNames, type ListsTransport } from "@shop/names/api";

/** A lists transport over fetch. An error's body is let go unread, so its status refuses the answer at once. */
export function listsTransport(fetchImpl: typeof fetch = (...a) => fetch(...a)): ListsTransport {
  return async (url, init) => {
    const res = await fetchImpl(url, init);
    if (!res.ok) {
      // The unread body is released, not left holding the connection; a stream that already failed can't be,
      // and that mustn't replace the status.
      void res.body?.cancel().catch(() => {});
      return { status: res.status, body: "" };
    }
    return { status: res.status, body: await res.text() };
  };
}

/** The banned-names list from this host's node, lowercased. */
export function readBannedNames(signal?: AbortSignal): Promise<string[]> {
  return fetchBannedNames({ base: catalystBase(), transport: listsTransport(), signal });
}
