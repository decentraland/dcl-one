import { useMemo } from "react";
import { Link, useSearchParams } from "react-router";

import Page from "@ui/shop/pages/Names";

import { track } from "@/core/lib/telemetry/track";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.names";

// No clientLoader: the server loader also answers client navigations here, for the telemetry session id.
export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("names", request, params);
export const meta = shopMeta("names");
export const headers = shopHeaders;

const DEPLOY_CONTEXT_KEYS = ["project", "world", "origin"] as const;

const RETURN_BAR = { display: "flex", alignItems: "center", padding: "12px 20px", borderBottom: "1px solid var(--line, rgba(255,255,255,0.18))" } as const;
const RETURN_LINK = { display: "inline-flex", alignItems: "center", minHeight: 44, color: "var(--brand, #ff2d55)", textDecoration: "none", fontSize: 14, fontWeight: 600 } as const;

/**
 * The Creator Hub's publish-to-World flow sends a creator without a NAME here (`from=deploy-world`,
 * with its project, World and origin). The return keeps that context; with a registered NAME it also
 * names it, and the publish flow preselects it only once the wallet owns it.
 */
export function returnToPublishHref(searchParams: URLSearchParams, worldName?: string): string {
  const params = new URLSearchParams();
  if (worldName) {
    params.set("name", worldName);
    params.set("claimed", "1");
  }
  for (const key of DEPLOY_CONTEXT_KEYS) {
    const value = searchParams.get(key)?.trim();
    if (value) params.set(key === "origin" ? "from" : key, value);
  }
  const qs = params.toString();
  return `/create/deploy-world${qs ? `?${qs}` : ""}`;
}

export default function ShopNamesRoute({ loaderData }: Route.ComponentProps) {
  const [searchParams] = useSearchParams();
  const fromDeploy = searchParams.get("from") === "deploy-world";
  const sid = loaderData?.sid ?? "";
  const modal = useMemo(
    () =>
      fromDeploy
        ? {
            use: {
              label: "Use in Publish to World",
              href: (name: string) => returnToPublishHref(searchParams, `${name.toLowerCase()}.dcl.eth`),
              onUse: (name: string) =>
                track(
                  "ch_claim_name_returned_to_publish",
                  { name, world_name: `${name.toLowerCase()}.dcl.eth` },
                  { sid, story: "creator-hub/claim-name" },
                ),
            },
          }
        : undefined,
    [fromDeploy, searchParams, sid],
  );
  return (
    <ShopPage data={loaderData}>
      {fromDeploy ? (
        <nav className="shop-names__return" aria-label="Back to publishing" style={RETURN_BAR}>
          <Link to={returnToPublishHref(searchParams)} style={RETURN_LINK}>
            {"\u{2190}"} Back to publishing your scene
          </Link>
        </nav>
      ) : null}
      <Page modal={modal} />
    </ShopPage>
  );
}
