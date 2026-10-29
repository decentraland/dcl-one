import Page from "@ui/shop/pages/ItemDetail";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.token.$contractAddress.$tokenId";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("tokenDetail", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("tokenDetail");
export const headers = shopHeaders;
// `Accept: text/markdown` (or ?format=md) answers the page as markdown for agents.
export const handle = { agentMarkdown: "shopPage" } satisfies AgentMarkdownHandle;

export default function ShopTokenRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
