// from: marketplace@3bf3662f:webapp/src/components/AccountSidebar/{AccountSidebar.css,OtherAccountSidebar/OtherAccountSidebar.tsx},
//       webapp/src/components/Vendor/decentraland/NFTSections/NFTSectionsMenuItems.tsx, webapp/src/components/AssetFilters
//       (the Rarity box), webapp/src/themes/shop-parity.css ("My Assets rail")
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import Icon from "../../../discovery/components/Icon";
import type { IconName } from "../../../discovery/components/Icon";
import { RARITIES, rarityColor, rarityLabel } from "../../../discovery/lib/rarity";
import { WEARABLE_SECTIONS, isWearableSection, type AssetType, type Section } from "./api";
import assetsIcon from "./icons/assets.svg?url";
import sparklesIcon from "./icons/sparkles.svg?url";

const NFT_SECTIONS: { key: Section; label: string }[] = [
  { key: "all", label: "All Assets" },
  { key: "wearables", label: "Wearables" },
  { key: "emotes", label: "Emotes" },
  { key: "ens", label: "Names" },
  { key: "land", label: "Land" },
];
const ITEM_SECTIONS: { key: Section; label: string }[] = [
  { key: "wearables", label: "Wearables" },
  { key: "emotes", label: "Emotes" },
];
const RARITY_ORDER = ["common", "uncommon", "rare", "epic", "legendary", "exotic", "mythic", "unique"].filter((r) => RARITIES.includes(r));

type SidebarProps = {
  assetType: AssetType;
  section: Section;
  rarities: string[];
  onBrowse: (next: { assetType?: AssetType; section: Section }) => void;
  onRarities: (next: string[]) => void;
};

function Item({ active, sub, icon, label, onClick }: { active: boolean; sub?: boolean; icon?: string; label: string; onClick: () => void }) {
  return (
    <li>
      <button type="button" className="acct-rail__item" data-sub={sub || undefined} aria-current={active ? "page" : undefined} onClick={onClick}>
        {icon ? <Icon name={icon as IconName} size={12} className="acct-rail__caticon" /> : null}
        {label}
      </button>
    </li>
  );
}

export default function AccountSidebar({ assetType, section, rarities, onBrowse, onRarities }: SidebarProps) {
  const sections = assetType === "item" ? ITEM_SECTIONS : NFT_SECTIONS;
  const showRarity = section !== "on_rent" && (isWearableSection(section) || section === "emotes");
  return (
    <nav className="acct-rail" aria-label="Account sections">
      <div className="acct-rail__menu acct-rail__menu--types">
        <button type="button" className="acct-rail__alt" aria-pressed={assetType === "item"} onClick={() => onBrowse({ assetType: "item", section: "wearables" })}>
          <span className="acct-rail__alticon acct-rail__alticon--sparkles" style={{ backgroundImage: `url("${sparklesIcon}")` }} aria-hidden="true" />
          <span>
            <span className="acct-rail__main">Originals</span>
            <span className="acct-rail__detail">Created by the user</span>
          </span>
        </button>
        <button type="button" className="acct-rail__alt" aria-pressed={assetType === "nft"} onClick={() => onBrowse({ assetType: "nft", section: "all" })}>
          <span className="acct-rail__alticon" style={{ backgroundImage: `url("${assetsIcon}")` }} aria-hidden="true" />
          <span>
            <span className="acct-rail__main">Collectibles</span>
            <span className="acct-rail__detail">Collected by the user</span>
          </span>
        </button>
      </div>
      <div className="acct-rail__menu">
        <h2 className="acct-rail__header">{assetType === "item" ? "CATEGORIES" : "ASSETS"}</h2>
        <ul>
          {sections.map((s) => (
            <SectionItems key={s.key} entry={s} section={section} onClick={(key) => onBrowse({ section: key })} />
          ))}
        </ul>
      </div>
      <div className="acct-rail__menu">
        <h2 className="acct-rail__header">ASSETS ON RENT</h2>
        <ul>
          <Item active={section === "on_rent"} label="Land" onClick={() => onBrowse({ assetType: "nft", section: "on_rent" })} />
        </ul>
      </div>
      {showRarity ? (
        <div className="acct-rail__menu acct-rail__filter">
          <h2 className="acct-rail__filtertitle">Rarity</h2>
          <div className="acct-rail__rarities" role="group" aria-label="Rarity">
            {RARITY_ORDER.map((r) => (
              <button
                key={r}
                type="button"
                className="acct-rail__rarity"
                aria-pressed={rarities.includes(r)}
                onClick={() => onRarities(rarities.includes(r) ? rarities.filter((x) => x !== r) : [...rarities, r])}
              >
                <span className="acct-rail__swatch" style={{ background: rarityColor(r) }} aria-hidden="true" />
                {rarityLabel(r)}
              </button>
            ))}
          </div>
        </div>
      ) : null}
    </nav>
  );
}

function SectionItems({ entry, section, onClick }: { entry: { key: Section; label: string }; section: Section; onClick: (s: Section) => void }) {
  const open = entry.key === "wearables" && isWearableSection(section);
  return (
    <>
      <Item active={section === entry.key} label={entry.label} onClick={() => onClick(entry.key)} />
      {open
        ? WEARABLE_SECTIONS.map((w) => <Item key={w.key} sub icon={w.icon} active={section === w.key} label={w.label} onClick={() => onClick(w.key)} />)
        : null}
    </>
  );
}
