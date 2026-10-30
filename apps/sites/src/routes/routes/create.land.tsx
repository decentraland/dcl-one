import { useState } from "react";
import type { CSSProperties } from "react";
import { Link, useNavigate, useRevalidator } from "react-router";

import Button from "@ui/atoms/Button";

import { PageHeader } from "@ui/components/Surface";
import CreatorHubChrome from "@ui/creatorhub/frames/CreatorHubChrome";
import EmptyState from "@ui/components/EmptyState";
import "@ui/creatorhub/pages/chmanage.css";
import {
  ManageError,
  ManageLoading,
  ManageSignIn,
  useManageScope,
} from "@/features/components/creator-hub/manage-scope";
import {
  loadLandList,
  type DelegatedParcel,
  type OwnedLand,
} from "@/data/lib/catalyst/creator-hub/land-list";
import { readWallet } from "@/data/lib/auth/wallet-cookie";
import { creatorHubMeta } from "@/core/lib/seo/creator-hub-meta";
import { LAND_CONTRACT } from "@ui/data/catalyst/landListing";

import type { Route } from "./+types/create.land";

export const meta = () => creatorHubMeta("LAND");

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const address = url.searchParams.get("address")?.trim().toLowerCase() || readWallet(request) || "";
  const list = address
    ? await loadLandList(address, { signal: request.signal }).catch(() => null)
    : { owned: [], delegated: [] };
  return {
    address,
    owned: list?.owned ?? [],
    delegated: list?.delegated ?? [],
    fallback: list === null,
  };
}

type LoaderData = {
  address: string;
  owned: OwnedLand[];
  delegated: DelegatedParcel[];
  fallback: boolean;
};

const LandGlyph = (
  <svg viewBox="0 0 20 20" width="28" height="28" fill="none" aria-hidden="true">
    <rect x="3" y="3" width="14" height="14" rx="1.3" stroke="currentColor" strokeWidth="1.5" />
    <path d="M7.7 3v14M12.3 3v14M3 7.7h14M3 12.3h14" stroke="currentColor" strokeWidth="1.5" />
  </svg>
);

const CARD_ACTION: CSSProperties = { textDecoration: "none" };
const PAGE = 60;

function shortAddress(addr: string): string {
  return addr.length > 12 ? `${addr.slice(0, 6)}\u{2026}${addr.slice(-4)}` : addr;
}

export default function CreateLand({ loaderData }: Route.ComponentProps) {
  return <LandView {...loaderData} />;
}

function LandView({ address, owned, delegated, fallback }: LoaderData) {
  const { rescoping, needsConnect } = useManageScope(address);
  const revalidator = useRevalidator();
  const navigate = useNavigate();
  const [visible, setVisible] = useState(PAGE);
  const total = owned.length + delegated.length;
  const shownOwned = owned.slice(0, visible);
  const shownDelegated = delegated.slice(0, Math.max(0, visible - owned.length));

  function onRetry() {
    if (typeof revalidator.revalidate === "function") revalidator.revalidate();
    else navigate(0);
  }

  return (
    <CreatorHubChrome active="land">
      <section className="chm">
        <div className="chm__container">
          <PageHeader
            title="Your LAND"
            description="Genesis City parcels this wallet owns, parcels inside its estates, and parcels other owners let it publish to."
          />
          <div className="chm__row">
            <div className="chm__content">
              {rescoping ? (
                <ManageLoading label="Loading your LAND\u{2026}" />
              ) : needsConnect ? (
                <ManageSignIn
                  icon={LandGlyph}
                  title="Sign in to manage your LAND"
                  subtitle="Sign in to see the parcels you own and the parcels you have publishing rights on. Scenes publish to LAND from the scene editor."
                />
              ) : (
                <>
                  {fallback ? (
                    <ManageError what="LAND" onRetry={onRetry} retrying={revalidator.state === "loading"} />
                  ) : null}
                  <div className="chm__filtersbar">
                    <div className="chm__filtersleft">
                      <h2 className="chm__count">
                        {total} {total === 1 ? "parcel" : "parcels"}
                      </h2>
                    </div>
                  </div>
                  {total === 0 && !fallback ? (
                    <EmptyState
                      className="es--card"
                      icon={LandGlyph}
                      iconWash
                      title="No LAND to display"
                      subtitle="This wallet owns no Genesis City parcels and has no parcel publishing grants. Every NAME you own comes with a World to publish to instead."
                      actions={
                        <>
                          <Link to="/shop/lands" prefetch="intent" className="btn btn--secondary btn--md" style={{ textDecoration: "none" }}>
                            Browse LAND
                          </Link>
                          <Link to="/create/worlds" prefetch="intent" className="btn btn--secondary btn--md" style={{ textDecoration: "none" }}>
                            Your Worlds
                          </Link>
                        </>
                      }
                      variant={undefined}
                      tone={undefined}
                      actionsGap={undefined}
                      style={undefined}
                    />
                  ) : (
                    <div className="chm__list">
                      {shownOwned.map((land) => (
                        <ParcelCard
                          key={`owned:${land.tokenId}`}
                          x={land.x}
                          y={land.y}
                          chips={[land.viaEstate ? "Estate" : "Owned"]}
                          label={land.viaEstate ? "Parcel in your estate" : "Owned parcel"}
                          shopHref={land.viaEstate ? undefined : `/shop/token/${LAND_CONTRACT}/${land.tokenId}/manage`}
                        />
                      ))}
                      {shownDelegated.map((land) => (
                        <ParcelCard
                          key={`grant:${land.id}`}
                          x={land.x}
                          y={land.y}
                          chips={["Publishing grant"]}
                          label={`Granted by ${shortAddress(land.owner)}`}
                        />
                      ))}
                    </div>
                  )}
                  {visible < total ? (
                    <div style={{ display: "flex", justifyContent: "center", margin: "20px 0 0" }}>
                      <Button variant="secondary" onClick={() => setVisible((n) => n + PAGE)}>
                        Show {Math.min(PAGE, total - visible)} more of {total - visible} remaining
                      </Button>
                    </div>
                  ) : null}
                </>
              )}
            </div>
          </div>
        </div>
      </section>
    </CreatorHubChrome>
  );
}

/** The Shop's manage page sells or rents out an owned parcel (the Shop owns sale and rental; this page owns scenes). */
function ParcelCard({ x, y, chips, label, shopHref }: { x: string; y: string; chips: string[]; label: string; shopHref?: string }) {
  const coords = `${x},${y}`;
  const scene = encodeURIComponent(`genesis:${coords}`);
  return (
    <div className="chm__card chm__card--compact">
      <div className="chm__cardhead chm__cardhead--chips">
        <span className="chm__cardtitle u-truncate">{coords}</span>
        <span className="chm__chips">
          {chips.map((chip) => (
            <span key={chip} className="chm__chip">{chip}</span>
          ))}
        </span>
      </div>
      <div className="chm__cardbody">
        <span className="chm__publabel">LAND</span>
        <span className="chm__projtitle">{label}</span>
        <div className="chm__cardactions">
          <Link
            to={`/play/?position=${encodeURIComponent(coords)}`}
            className="chm__settingsbtn"
            style={CARD_ACTION}
            reloadDocument
          >
            Jump in
          </Link>
          <Link to={`/create/scene-analytics?scene=${scene}`} prefetch="intent" className="chm__settingsbtn" style={CARD_ACTION}>
            Analytics
          </Link>
          <Link to={`/create/scene-errors?type=genesis&scene=${encodeURIComponent(coords)}`} prefetch="intent" className="chm__settingsbtn" style={CARD_ACTION}>
            Errors
          </Link>
          {shopHref ? (
            <Link to={shopHref} prefetch="intent" className="chm__settingsbtn" style={CARD_ACTION}>
              Sell or rent
            </Link>
          ) : null}
        </div>
      </div>
    </div>
  );
}
