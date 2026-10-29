// from: shop@2d3bf62:app/src/components/CampaignBanner/CampaignBanner.tsx, app/src/hooks/useCampaignHero.ts

import { useCampaign } from "../hooks";
import { assetUrl, localized, type ContentfulLocale } from "../lib/contentful";
import "./campaignbanner.css";

export type CampaignHero = { title: string; desktopImage: string; mobileImage: string; cta: { label: string; href: string } | null; bannerId: string; campaignName: string | null };

export function useCampaignHero(slot: string, locale: ContentfulLocale = "en-US"): CampaignHero | null {
  const { campaign } = useCampaign();
  const banner = campaign?.banners[slot];
  if (!campaign || !banner) return null;
  const title = localized(banner.desktopTitle, locale) ?? "";
  const desktopImage = assetUrl(campaign.assets, localized(banner.fullSizeBackground, locale));
  const mobileImage = assetUrl(campaign.assets, localized(banner.mobileBackground, locale)) || desktopImage;
  if (!title && !desktopImage) return null;
  const show = localized(banner.showButton, locale) === true;
  const href = localized(banner.buttonLink, locale);
  const label = localized(banner.buttonsText, locale);
  return { title, desktopImage, mobileImage, cta: show && href && label ? { href, label } : null, bannerId: banner.id, campaignName: campaign.name };
}

export default function CampaignBanner({ slot, iapMode = false }: { slot: string; iapMode?: boolean }) {
  const banner = useCampaignHero(slot);
  if (!banner) return null;
  return (
    <section
      className="shop-campaign"
      data-testid="campaign-banner"
      aria-label={banner.title}
      style={banner.desktopImage ? ({ ["--banner-art" as string]: `url("${banner.desktopImage.replace(/"/g, "%22")}")` }) : undefined}
    >
      {banner.desktopImage ? (
        <picture>
          <source media="(max-width: 768px)" srcSet={banner.mobileImage} />
          <img className="shop-campaign__bg" src={banner.desktopImage} alt="" />
        </picture>
      ) : null}
      <div className="shop-campaign__inner">
        <h2 data-testid="campaign-banner-title">{banner.title}</h2>
        {!iapMode && banner.cta ? (
          <a className="shop-campaign__cta" href={banner.cta.href} data-testid="campaign-banner-cta">
            {banner.cta.label}
          </a>
        ) : null}
      </div>
    </section>
  );
}
