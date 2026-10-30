// Moved from catalyrst-v2 ui3/src/marketplace/components/EnsCard.tsx (since removed).
// from: marketplace@f3956826:webapp/src/components/AssetImage/EnsImage/{EnsImage.tsx,utils.ts} (the
// length-banded gradients of a NAME's art)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// One NAME as a card: its art, `name.dcl.eth`, the network and, when it is on resale, its MANA price.
// A NAME resale is MANA only, so a card never shows a credit price.

import { Link } from "react-router";

import DclLogomark from "../../atoms/DclLogomark";
import ManaMark from "../../atoms/ManaMark";
import { manaText } from "../land/format";
import "./namecard.css";

type Gradient = { min: number; max: number; c: [string, string] };

const GRADIENTS: Gradient[] = [
  { min: 2, max: 3, c: ["#C640CD", "#691FA9"] },
  { min: 4, max: 5, c: ["#FF2D55", "#FFBC5B"] },
  { min: 5, max: 6, c: ["#73FFAF", "#1A9850"] },
  { min: 7, max: 8, c: ["#81D1FF", "#3077E1"] },
  { min: 9, max: 10, c: ["#F6C1FF", "#FF4BED"] },
  { min: 11, max: 15, c: ["#FF9EB1", "#FF2D55"] },
];

export function nameGradient(name?: string): string {
  const n = (name || "").length;
  const g = GRADIENTS.find((x) => n >= x.min && n <= x.max) ?? { c: ["#000", "#fff"] as [string, string] };
  return `linear-gradient(135deg, ${g.c[0]} 0%, ${g.c[1]} 100%)`;
}

export type NameCardProps = {
  name: string;
  /** Resale price in MANA wei (exact); `price` is the legacy display string EnsCard took. */
  priceWei?: string | null;
  price?: string | number | null;
  href?: string;
  onClick?: () => void;
};

export default function NameCard({ name, priceWei, price, href, onClick }: NameCardProps) {
  const shown = priceWei != null ? manaText(priceWei) : price != null ? String(price) : null;
  const body = (
    <>
      <span className="shop-namecard__art" aria-hidden="true" style={{ background: nameGradient(name) }}>
        <span className="shop-namecard__mark">
          <DclLogomark size={40} />
        </span>
        <span className="shop-namecard__name">{name}</span>
        <span className="shop-namecard__sub">DCL.ETH</span>
      </span>
      <span className="shop-namecard__body">
        <span className="shop-namecard__row">
          <span className="shop-namecard__title u-truncate">{name}</span>
          {shown !== null ? (
            <span className="shop-namecard__price" aria-label={`${shown} MANA`}>
              <ManaMark size={13} className="shop-namecard__manamark" />
              {shown}
            </span>
          ) : null}
        </span>
        <span className="shop-namecard__meta">
          <span className="shop-namecard__network">Ethereum</span>
          <span className="shop-namecard__badge">Name</span>
        </span>
      </span>
    </>
  );
  if (href) {
    return (
      <Link className="shop-namecard enscard" to={href} aria-label={`${name}.dcl.eth${shown !== null ? `, ${shown} MANA` : ""}`}>
        {body}
      </Link>
    );
  }
  return (
    <button type="button" className="shop-namecard enscard" onClick={onClick}>
      {body}
    </button>
  );
}
