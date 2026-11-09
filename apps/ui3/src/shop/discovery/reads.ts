// The browser host's options for the Shop's reads (discovery/api, detail/api): the document's client and the
// creator-sales switch as this page holds it. The fetchers take them as arguments and read neither themselves.

import { shopClient } from "../api/host";
import type { Opts } from "./api";
import { shopFlags } from "./flags";

export function shopReads(signal?: AbortSignal): Opts {
  return { client: shopClient(), creatorSales: () => shopFlags().creatorSales, signal };
}
