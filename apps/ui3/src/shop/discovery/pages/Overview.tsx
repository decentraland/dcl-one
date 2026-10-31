// from: shop@2d3bf62:app/src/pages/Overview.tsx, app/src/pages/Overview.styles.ts;
//       marketplace@f3956826:webapp/src/components/Routes/Routes.tsx (`/`, the home page)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useEffect } from "react";
import { preload } from "react-dom";
import { Link } from "react-router";

import Button from "../../../atoms/Button";
import { SHOP_BASE } from "../../routes";
import { fetchShopItems, fetchTrendingItems } from "../api";
import { useCampaignHero } from "../components/CampaignBanner";
import heroDesktop from "../frames/assets/hero-credits-outfits.webp?url";
import heroMobile from "../frames/assets/hero-credits-mobile.webp?url";
import OverviewOutfitsRow from "./OverviewOutfitsRow";
import promoEmotes from "./assets/overview/promo-best-rated-emotes.webp?url";
import promoOutfits from "./assets/overview/promo-week-selected-outfits.webp?url";
import IndexNotice from "../components/IndexNotice";
import LivePromo from "../components/LivePromo";
import Rail from "../components/Rail";
import SuggestedForYouRow from "../components/SuggestedForYouRow";
import TopCreators from "../components/TopCreators";
import { indexUnavailable } from "../freshness";
import { isIapMode, shopFlags } from "../flags";
import { useListingsFreshness, useSeo } from "../hooks";
import { t } from "../intl";
import { useShopQuery } from "../query";
import "./overview.css";

const MIN_DEALS = 3;

function twoRows(s: string, words: number): string {
  if (s.includes("\n")) return s;
  const parts = s.split(" ");
  return parts.length > words ? `${parts.slice(0, words).join(" ")}\n${parts.slice(words).join(" ")}` : s;
}
const CAMPAIGN_HERO_SLOT = "marketplaceHomepageBanner";
const HOME_TITLE = "Decentraland Shop | Wearables & Emotes for Your Avatar";

function useHomeTitle() {
  useEffect(() => {
    if (typeof document === "undefined") return;
    const previous = document.title;
    document.title = HOME_TITLE;
    return () => {
      document.title = previous;
    };
  }, []);
}

function Hero() {
  const campaign = useCampaignHero(CAMPAIGN_HERO_SLOT);
  const iap = isIapMode();
  if (!campaign?.desktopImage) {
    preload(heroDesktop, { as: "image", fetchPriority: "high", media: "(min-width: 769px)" });
    preload(heroMobile, { as: "image", fetchPriority: "high", media: "(max-width: 768px)" });
  }
  return (
    <section className="shop-overview__hero" style={campaign?.desktopImage ? { backgroundImage: `url("${campaign.desktopImage}")` } : undefined}>
      <div className="shop-overview__hero-inner">
        <h1 data-testid="hero-title">{campaign?.title || t("overview.heroTitle")}</h1>
        {iap ? null : campaign ? (
          campaign.cta ? (
            <a className="shop-overview__cta" href={campaign.cta.href} data-testid="hero-campaign-cta">
              {campaign.cta.label}
            </a>
          ) : null
        ) : (
          <Link className="shop-overview__cta" to={`${SHOP_BASE}/credits`} data-testid="hero-credits-cta">
            {t("overview.heroCta")}
          </Link>
        )}
      </div>
    </section>
  );
}

export default function Overview() {
  useSeo({ title: null });
  useHomeTitle();
  const flags = shopFlags();
  const listingType = flags.secondarySales ? undefined : ("primary" as const);

  const listings = useShopQuery(["overview-listings"], (signal) => fetchShopItems({ first: 12, sortBy: "newest", listingType: "primary" }, { signal }));
  const trending = useShopQuery(["overview-trending", listingType ?? "all"], (signal) => fetchTrendingItems({ first: 12, listingType }, { signal }));
  const deals = useShopQuery(flags.creatorSales ? ["overview-deals"] : null, (signal) =>
    fetchShopItems({ first: 12, discounted: true, sortBy: "discount", listingType: "primary" }, { signal }),
  );

  const items = listings.data?.items ?? [];
  const trendingItems = trending.data ?? [];
  const dealItems = deals.data?.items ?? [];
  const unavailable = indexUnavailable(listings.error);
  const listingsFreshness = useListingsFreshness();

  return (
    <div className="shop-overview" data-testid="overview">
      <Hero />

      {dealItems.length >= MIN_DEALS ? <Rail title={t("overview.bestDeals")} items={dealItems} source="deals" viewAllTo={`${SHOP_BASE}/items?deals=true`} testId="best-deals-rail" /> : null}

      {trending.isLoading || trendingItems.length > 0 ? <Rail title={t("overview.trendingProducts")} items={trendingItems} loading={trending.isLoading} source="trending" viewAllTo={`${SHOP_BASE}/items`} testId="trending-rail" /> : null}

      <OverviewOutfitsRow />

      <SuggestedForYouRow />

      {unavailable ? (
        <IndexNotice freshness={unavailable} what={t("overview.newCreations")} onRetry={listings.refetch} />
      ) : listings.error ? (
        <div className="shop-overview__error" role="alert" data-testid="overview-error">
          <p>{t("overview.loadError")}</p>
          <Button variant="secondary" size="sm" onClick={listings.refetch}>
            {t("loadMore.retry")}
          </Button>
        </div>
      ) : listings.isLoading || items.length > 0 ? (
        <>
          <IndexNotice freshness={listings.data?.meta.freshness ?? listingsFreshness} what={t("overview.newCreations")} />
          <Rail title={t("overview.newCreations")} items={items.slice(0, 12)} loading={listings.isLoading} source="new_creations" viewAllTo={`${SHOP_BASE}/items`} />
          <div className="shop-overview__promos">
            <LivePromo
              to={`${SHOP_BASE}/items?category=emote`}
              urns={["urn:decentraland:matic:collections-v2:0x0c956c74518ed34afb7b137d9ddfdaea7ca13751:0", "urn:decentraland:matic:collections-v2:0xe9f388ae27c726c4772c85a194e9791b1a0a913c:0"]}
              title={twoRows(t("overview.expressWithStyle"), 1)}
              cta={t("overview.exploreEmotes")}
              ariaLabel={t("overview.promoEmotesAria")}
              fallback={promoEmotes}
              fallbackAlt={t("overview.promoEmotesAria")}
              testId="promo-emotes"
            />
            <LivePromo
              to={`${SHOP_BASE}/items?category=wearable`}
              urns={["urn:decentraland:matic:collections-v2:0x9620151fe5e1c8fd0638a4840cf5e63d19b09765:0", "urn:decentraland:matic:collections-v2:0x6c3ca91dbac390d60d4267fdcf48576f6c051dbe:0"]}
              title={twoRows(t("overview.findYourLook"), 2)}
              cta={t("overview.exploreWearables")}
              ariaLabel={t("overview.promoOutfitsAria")}
              fallback={promoOutfits}
              fallbackAlt={t("overview.promoOutfitsAria")}
              testId="promo-wearables"
            />
          </div>
        </>
      ) : (
        <div className="shop-overview__empty" data-testid="overview-empty">
          <h2>{t("overview.emptyTitle")}</h2>
          <p>{t("overview.emptyBody")}</p>
          <Link className="shop-overview__cta" to={`${SHOP_BASE}/items`}>
            {t("notFound.cta")}
          </Link>
        </div>
      )}

      <TopCreators />
    </div>
  );
}

