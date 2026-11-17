// from: marketplace@f3956826:webapp/src/components/NamesPage/{NamesPage,ClaimNamePage/ClaimNamePage}.tsx
// (the two NAME routes: /names/claim and /names/browse), shop@2d3bf62:app/src/pages/NamesPage.tsx,
// shop@2d3bf62:app/src/pages/Assets.tsx (the collectibles grid's useSeo, which wins over NamesPage's)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
import { useSearchParams } from "react-router";

import type { ShopClient } from "@shop/api/shop";
import { useSeo } from "../discovery/hooks";
import { useMarketplaceAccess } from "../market/access";
import NamesBrowse from "./NamesBrowse";
import NamesPage, { type NamesPageProps } from "./NamesPage";
import "./names.css";

const CLAIM_SEO = {
  title: "Browse Collectibles",
  description: "Browse thousands of wearables and emotes for your Decentraland avatar. Filter by category, rarity, and price to find your next look.",
};

export type NamesHubProps = NamesPageProps & { client?: ShopClient };

export default function NamesHub(props: NamesHubProps) {
  const [params] = useSearchParams();
  // NAME resale is the Marketplace's (market/access.tsx): without it, the claim page answers every NAMEs link. While
  // the answer is pending a resale link waits, rather than mounting the claim page (its SEO and reads) first.
  const access = useMarketplaceAccess();
  const asked = params.get("section") === "resale";
  const resale = access === "on" && asked;
  const iapMode = props.modal?.iapMode ?? params.get("view") === "mobile-iap";
  useSeo(resale || (asked && access === "pending") ? {} : CLAIM_SEO);
  if (asked && access === "pending") return <div className="shop-names" data-testid="names-pending" aria-busy="true" />;
  return (
    <div className={resale ? "shop-names" : "shop-names shop-names--claim"}>
      {resale ? <NamesBrowse client={props.client} /> : <NamesPage {...props} modal={{ ...props.modal, iapMode }} />}
    </div>
  );
}
