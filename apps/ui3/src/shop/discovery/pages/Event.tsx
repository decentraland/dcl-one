// from: shop@2d3bf62:app/src/pages/Event.tsx (upstream also passes itemIds={campaign.items} and the
//       seo description; those wait on Items props — pending. noindex is applied here);
//       marketplace@f3956826:webapp/src/components/Routes/Routes.tsx (`/campaign`), webapp/src/modules/event/actions.ts (FETCH_EVENT_REQUEST)
// Licence of the Marketplace parts: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.

import { useEffect } from "react";
import { Navigate } from "react-router";

import Spinner from "../../../atoms/Spinner";
import { SHOP_BASE } from "../../routes";
import CampaignBanner from "../components/CampaignBanner";
import { useCampaign, useCampaignContracts } from "../hooks";
import { t } from "../intl";
import { localized, toContentfulLocale } from "../lib/contentful";
import Items from "./Items";

const CAMPAIGN_BANNER_SLOT = "marketplaceCampaignCollectiblesBanner";
const SEO_FALLBACK_TITLE = "Browse Collectibles";

function useNoindex(): void {
  useEffect(() => {
    if (typeof document === "undefined") return;
    const meta = document.createElement("meta");
    meta.name = "robots";
    meta.content = "noindex";
    document.head.appendChild(meta);
    return () => meta.remove();
  }, []);
}

export default function Event() {
  useNoindex();
  const { campaign, isPending: campaignPending } = useCampaign();
  const { contracts, isPending: contractsPending } = useCampaignContracts(campaign?.tags ?? [], campaign?.collections);
  const gone = !campaignPending && !campaign?.mainTag;

  if (gone) return <Navigate to={`${SHOP_BASE}/items`} replace />;
  if (campaignPending || contractsPending || contracts === undefined) {
    return (
      <div className="page-loading" aria-busy="true" data-testid="event-loading" style={{ display: "grid", placeItems: "center", minHeight: "60vh" }}>
        <span style={{ marginTop: 14, display: "inline-flex" }}>
          <Spinner size={30} aria-label={t("event.loading")} />
        </span>
      </div>
    );
  }
  const name = localized(campaign?.tabName, toContentfulLocale("en")) ?? campaign?.name ?? "";
  return (
    <>
      <CampaignBanner slot={CAMPAIGN_BANNER_SLOT} />
      <Items contracts={contracts} hideNames lockStatus="on_sale" seo={{ title: name || SEO_FALLBACK_TITLE }} />
    </>
  );
}
