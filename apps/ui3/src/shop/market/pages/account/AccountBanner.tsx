// from: marketplace@3bf3662f:webapp/src/components/AccountPage/AccountBanner/AccountBanner.{tsx,css}
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";

import { copyText } from "../../../../data/clipboard";
import { fetchProfile, profileFaceUrl } from "../../../../data/catalyst/profile";
import { shopPath } from "../../../routes";
import { fetchStore } from "../../../seller/lib/store";
import discordIcon from "./icons/icon-discord.svg?url";
import facebookIcon from "./icons/icon-facebook.svg?url";
import twitterIcon from "./icons/icon-twitter.svg?url";
import websiteIcon from "./icons/icon-website.svg?url";

const LINK_ICONS = [
  ["website", websiteIcon],
  ["facebook", facebookIcon],
  ["twitter", twitterIcon],
  ["discord", discordIcon],
] as const;

function shortenAddress(address: string): string {
  return `${address.slice(0, 6)}...${address.slice(-5)}`;
}

function Blockie({ address }: { address: string }) {
  const hue = parseInt(address.slice(2, 8), 16) % 360;
  return <span className="acct-banner__blockie" style={{ background: `linear-gradient(135deg, hsl(${hue} 70% 55%), hsl(${(hue + 60) % 360} 70% 40%))` }} aria-hidden="true" />;
}

export default function AccountBanner({ address }: { address: string }) {
  const navigate = useNavigate();
  const [copied, setCopied] = useState(false);
  const profile = useQuery({ queryKey: ["profile", address], queryFn: () => fetchProfile(address), retry: false, staleTime: 5 * 60_000 });
  const store = useQuery({ queryKey: ["store", address], queryFn: () => fetchStore(address), staleTime: 5 * 60_000 });
  useEffect(() => {
    if (!copied) return;
    const id = setTimeout(() => setCopied(false), 1200);
    return () => clearTimeout(id);
  }, [copied]);
  const face = profileFaceUrl(profile.data ?? null);
  const name = profile.data?.name || shortenAddress(address);
  const links = store.data?.links;
  const onBack = () => {
    if (typeof window !== "undefined" && window.history.length > 1) navigate(-1);
    else navigate(shopPath("overview"));
  };
  const copy = async () => {
    if (await copyText(address)) setCopied(true);
  };
  return (
    <header className="acct-banner" data-testid="shop-account-banner">
      {store.data?.cover ? <img className="acct-banner__cover" src={store.data.cover} alt="" /> : null}
      <div className="acct-banner__top">
        <button type="button" className="acct-banner__back" aria-label="Back" onClick={onBack}>
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
            <path d="M15 5l-7 7 7 7" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>
        <div className="acct-banner__links">
          {links
            ? LINK_ICONS.filter(([k]) => /^https?:\/\//i.test(links[k] ?? "")).map(([k, icon]) => (
                <a key={k} className="acct-banner__link" href={links[k]} target="_blank" rel="noopener noreferrer nofollow" aria-label={k} style={{ backgroundImage: `url("${icon}")` }} />
              ))
            : null}
        </div>
      </div>
      <div className="acct-banner__column">
        {face ? <img className="acct-banner__face" src={face} alt="" /> : <Blockie address={address} />}
        <h1 className="acct-banner__name">
          <a href={`/profile/${address}`} target="_blank" rel="noopener">
            {name}
          </a>
        </h1>
        <div className="acct-banner__address">
          <span className="acct-banner__hash" title={address}>
            {shortenAddress(address)}
          </span>
          <button type="button" className="acct-banner__copy" aria-label="Copy address" onClick={() => void copy()}>
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
              <rect x="5" y="5" width="8" height="9" rx="1.2" fill="none" stroke="currentColor" strokeWidth="1.5" />
              <path d="M3 11V3.2C3 2.5 3.5 2 4.2 2H10" fill="none" stroke="currentColor" strokeWidth="1.5" />
            </svg>
          </button>
          {copied ? (
            <span className="acct-banner__copied" role="status">
              Copied!
            </span>
          ) : null}
        </div>
        {store.data?.description ? <p className="acct-banner__description">{store.data.description}</p> : null}
      </div>
    </header>
  );
}
