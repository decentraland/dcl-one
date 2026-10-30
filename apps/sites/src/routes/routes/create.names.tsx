import { Link, useNavigate, useRevalidator } from "react-router";
import type { CSSProperties } from "react";

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
import { loadNamesList } from "@/data/lib/catalyst/creator-hub/names-list.server";
import type { OwnedNameVM } from "@/data/lib/catalyst/creator-hub/names-list";
import { readWallet } from "@/data/lib/auth/wallet-cookie";
import { creatorHubMeta } from "@/core/lib/seo/creator-hub-meta";

import type { Route } from "./+types/create.names";

export const meta = () => creatorHubMeta("NAMEs");

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const address = url.searchParams.get("address")?.trim().toLowerCase() || readWallet(request) || "";
  const names = address ? await loadNamesList(address, request.signal).catch(() => null) : [];
  return { address, names: names ?? [], fallback: names === null };
}

type LoaderData = { address: string; names: OwnedNameVM[]; fallback: boolean };

const NameGlyph = (
  <svg viewBox="0 0 20 20" width="28" height="28" fill="none" aria-hidden="true">
    <path d="M10.3 3H16a1 1 0 0 1 1 1v5.7a1 1 0 0 1-.3.7l-6 6a1 1 0 0 1-1.4 0L3.6 11.7a1 1 0 0 1 0-1.4l6-6a1 1 0 0 1 .7-.3Z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
    <circle cx="13.3" cy="6.7" r="1.1" fill="currentColor" />
  </svg>
);

const CARD_ACTION: CSSProperties = { textDecoration: "none" };
const CLAIM_HREF = "/shop/names?from=names";

export default function CreateNames({ loaderData }: Route.ComponentProps) {
  return <NamesView {...loaderData} />;
}

function NamesView({ address, names, fallback }: LoaderData) {
  const { rescoping, needsConnect } = useManageScope(address);
  const revalidator = useRevalidator();
  const navigate = useNavigate();

  function onRetry() {
    if (typeof revalidator.revalidate === "function") revalidator.revalidate();
    else navigate(0);
  }

  return (
    <CreatorHubChrome active="names">
      <section className="chm">
        <div className="chm__container">
          <PageHeader
            title="Your NAMEs"
            description="Every NAME you own is your avatar's name in-world and comes with a World of its own to publish scenes to."
            style={{ alignItems: "flex-start" }}
            actions={
              address ? (
                <Link to={CLAIM_HREF} prefetch="intent" className="btn btn--primary btn--md" style={{ textDecoration: "none" }}>
                  Claim a new NAME
                </Link>
              ) : undefined
            }
          />
          <div className="chm__row">
            <div className="chm__content">
              {rescoping ? (
                <ManageLoading label="Loading your NAMEs\u{2026}" />
              ) : needsConnect ? (
                <ManageSignIn
                  icon={NameGlyph}
                  title="Sign in to manage your NAMEs"
                  subtitle="Sign in to see the NAMEs you own and the World each one entitles. A new NAME costs 100 MANA."
                />
              ) : (
                <>
                  {fallback ? (
                    <ManageError what="NAMEs" onRetry={onRetry} retrying={revalidator.state === "loading"} />
                  ) : null}
                  <div className="chm__filtersbar">
                    <div className="chm__filtersleft">
                      <h2 className="chm__count">
                        {names.length} {names.length === 1 ? "NAME" : "NAMEs"}
                      </h2>
                    </div>
                  </div>
                  {names.length === 0 && !fallback ? (
                    <EmptyState
                      className="es--card"
                      icon={NameGlyph}
                      iconWash
                      title="No NAMEs to display"
                      subtitle="Mint a NAME to name your avatar and get a World to publish Scenes to."
                      actions={
                        <Link to={CLAIM_HREF} prefetch="intent" className="btn btn--secondary btn--md" style={{ textDecoration: "none" }}>
                          Claim a NAME
                        </Link>
                      }
                      variant={undefined}
                      tone={undefined}
                      actionsGap={undefined}
                      style={undefined}
                    />
                  ) : (
                    <div className="chm__list">
                      {names.map((entry) => (
                        <NameCard key={entry.worldName} entry={entry} address={address} />
                      ))}
                    </div>
                  )}
                </>
              )}
            </div>
          </div>
        </div>
      </section>
    </CreatorHubChrome>
  );
}



function NameCard({ entry, address }: { entry: OwnedNameVM; address: string }) {
  const published = entry.deployedScenes > 0;
  const settingsHref = `/create/world-settings?world=${encodeURIComponent(entry.worldName)}&address=${encodeURIComponent(address)}`;
  return (
    <div className="chm__card chm__card--compact">
      <div className="chm__cardhead chm__cardhead--chips">
        <span className="chm__cardtitle u-truncate">{entry.name}</span>
        <span className="chm__chips">
          <span className="chm__chip">
            {published
              ? `${entry.deployedScenes} ${entry.deployedScenes === 1 ? "scene" : "scenes"}`
              : "World not published"}
          </span>
        </span>
      </div>
      <div className="chm__cardbody">
        <span className="chm__publabel">World</span>
        <span className="chm__projtitle">{entry.title ?? entry.worldName}</span>
        <div className="chm__cardactions">
          <Link to={settingsHref} prefetch="intent" className="chm__settingsbtn" style={CARD_ACTION}>
            World settings
          </Link>
          {entry.contractAddress && entry.tokenId ? (
            <Link to={`/shop/token/${entry.contractAddress.toLowerCase()}/${entry.tokenId}/sell`} prefetch="intent" className="chm__settingsbtn" style={CARD_ACTION}>
              Sell
            </Link>
          ) : null}
          {published ? (
            <>
              <Link
                to={`/create/scene-analytics?scene=${encodeURIComponent(`world:${entry.worldName}`)}&src=names`}
                prefetch="intent"
                className="chm__settingsbtn"
                style={CARD_ACTION}
              >
                Analytics
              </Link>
              <Link
                to={`/play/?realm=${encodeURIComponent(entry.worldName)}`}
                className="chm__settingsbtn"
                style={CARD_ACTION}
                reloadDocument
              >
                Jump in
              </Link>
            </>
          ) : (
            <Link to="/create/scenes" prefetch="intent" className="chm__settingsbtn" style={CARD_ACTION}>
              Publish a scene
            </Link>
          )}
        </div>
      </div>
    </div>
  );
}
