// from: shop@2d3bf62:app/src/components/CollectionCreatorCard/CollectionCreatorCard.tsx,
//       app/src/components/CopyButton/CopyButton.tsx, app/src/assets/icons/copy.svg

import { useEffect, useRef, useState } from "react";

import { copyText } from "../../../../data/clipboard";
import { shortAddress } from "../../../model/address";
import Icon from "../../components/Icon";
import { useProfile } from "../../hooks";
import { getAvatarBackgroundColor, getDisplayName } from "../../lib/avatarColor";

const COPY = "Copy account";
const COPIED = "Copied";
const VIEW_PROFILE = "View profile";

function CopyAddress({ address }: { address: string }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => void (timer.current && clearTimeout(timer.current)), []);
  const title = copied ? COPIED : COPY;
  return (
    <button
      type="button"
      className="shop-ccard__copy"
      title={title}
      aria-label={title}
      data-copied={copied || undefined}
      data-testid="creator-card-copy"
      onClick={() =>
        void copyText(address).then((ok) => {
          if (!ok) return;
          setCopied(true);
          if (timer.current) clearTimeout(timer.current);
          timer.current = setTimeout(() => setCopied(false), 1500);
        })
      }
    >
      <span>{shortAddress(address)}</span>
      <Icon key={copied ? "ok" : "idle"} name={copied ? "check" : "copy"} size={16} className="shop-ccard__glyph" />
    </button>
  );
}

export default function CreatorCard({ address }: { address?: string | null }) {
  const profile = useProfile(address);
  if (!address) return null;
  const name = profile?.name || shortAddress(address);
  const bg = getAvatarBackgroundColor(getDisplayName(profile?.name, profile?.claimed, address));
  return (
    <div className="shop-ccard" data-testid="creator-card">
      {profile?.picture ? (
        <img className="shop-ccard__ava" src={profile.picture} alt="" loading="eager" style={{ backgroundColor: bg }} />
      ) : (
        <span className="shop-ccard__ava" aria-hidden style={{ backgroundColor: bg }} />
      )}
      <h2 className="shop-ccard__name" title={name}>
        {name}
      </h2>
      <CopyAddress address={address} />
      <a className="shop-ccard__view" data-testid="creator-card-view" href={`/profile/${address}`} target="_blank" rel="noopener noreferrer">
        {VIEW_PROFILE}
      </a>
    </div>
  );
}
