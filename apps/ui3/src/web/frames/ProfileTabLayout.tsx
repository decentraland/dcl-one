import ProfileNavigation from "./ProfileNavigation";
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { Copy, WalletGlyph, VerifiedGlyph, GlobeBtnGlyph, PersonAddGlyph, MoreGlyph } from "../../atoms/icons";
import { suffixLabel, type LabelSuffixProps } from "../../components/labelSuffix";
import "./profiletablayout.css";
import { hueFor, truncateAddress } from "../../data/format";

type Profile = {
  address: string;
  name?: string | null;
  nameColor?: string;
  avatarUrl?: string;
  hasClaimedName?: boolean;
};

type Tab = { id: string; label: ReactNode };

export const MEMBER_TABS: { id: string; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "creations", label: "Creations" },
  { id: "communities", label: "Communities" },
  { id: "places", label: "Places" },
  { id: "photos", label: "Photos" },
];
export const OWN_TABS: { id: string; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "assets", label: "My Assets" },
  { id: "communities", label: "My Communities" },
  { id: "places", label: "My Places" },
  { id: "photos", label: "My Photos" },
  { id: "referral-rewards", label: "Referral Rewards" },
];

const MUTUAL_COLORS = ["#73d3d3", "#ff8362", "#b05cff"];

/** Header actions: the name/world CTA on your own profile; mutuals, add friend and more on anyone else's. */
export function ProfileActions({ isOwnProfile, hasClaimedName, mutualCount = 0 }: {
  isOwnProfile?: boolean;
  hasClaimedName?: boolean;
  mutualCount?: number;
}) {
  if (isOwnProfile) {
    return (
      <button type="button" className="ptl__namecta">
        {hasClaimedName ? <GlobeBtnGlyph /> : <VerifiedGlyph />}
        {hasClaimedName ? "Manage World" : "Get a Unique Name"}
      </button>
    );
  }
  return (
    <>
      {mutualCount > 0 ? (
        <button type="button" className="ptl__mutual" aria-label={`${mutualCount} Mutual`}>
          <span className="ptl__mutualstack">
            {Array.from({ length: Math.min(3, mutualCount) }).map((_, i) => (
              <span key={i} className="ptl__mutualpic" style={{ background: MUTUAL_COLORS[i], marginLeft: i ? -8 : 0 }} aria-hidden="true" />
            ))}
          </span>
          <span className="ptl__mutualtext">
            <strong>{mutualCount}</strong> Mutual
          </span>
        </button>
      ) : null}
      <button type="button" className="ptl__cta">
        <PersonAddGlyph />
        Add friend
      </button>
      <button type="button" className="ptl__more" aria-label="More actions">
        <MoreGlyph />
      </button>
    </>
  );
}

type ProfileTabLayoutProps = LabelSuffixProps & {
  profile: Profile;
  tabs?: Tab[];
  activeTab?: string;
  onTabChange?: (id: string) => void;
  actions?: ReactNode;
  aside?: ReactNode;
  onCopyAddress?: () => void;
  children?: ReactNode;
  showWalletIcon?: boolean;
  showCopy?: boolean;
  bodyPadded?: boolean;
  pageBackground?: string;
  className?: string;
};

export default function ProfileTabLayout({
  labelSuffix,
  profile,
  tabs = [],
  activeTab,
  onTabChange,
  actions,
  aside,
  onCopyAddress,
  children,
  showWalletIcon = true,
  showCopy = true,
  bodyPadded = false,
  pageBackground,
  className,
}: ProfileTabLayoutProps) {
  const initial = (profile.name || profile.address).charAt(0).toUpperCase();
  const hasClaimedName = profile.hasClaimedName ?? false;
  const nameColor = profile.nameColor;
  const [failedAvatar, setFailedAvatar] = useState("");
  const [loadedAvatar, setLoadedAvatar] = useState("");
  const avatarRef = useRef<HTMLImageElement>(null);
  const avatarUrl = profile.avatarUrl !== failedAvatar ? profile.avatarUrl : undefined;

  useEffect(() => {
    // The SSR image may finish loading before React attaches its event handlers.
    const image = avatarRef.current;
    if (!avatarUrl || !image?.complete) return;
    if (image.naturalWidth) setLoadedAvatar(avatarUrl);
    else setFailedAvatar(avatarUrl);
  }, [avatarUrl]);

  const avatarStyle: CSSProperties = nameColor
    ? { background: nameColor }
    : ({ "--hue": hueFor(profile.name || profile.address || "") } as CSSProperties);
  const avatarClass = "ptl__avatar" + (!nameColor && !avatarUrl ? " u-avatar" : "");

  const pageStyle: CSSProperties | undefined = pageBackground ? { background: pageBackground } : undefined;

  return (
    <div className={"ptl" + (className ? " " + className : "")} style={pageStyle}>
      <div className="ptl__content">
        <div className="ptl__card">
          <section className="ptl__header" aria-label={suffixLabel("Profile summary", labelSuffix)}>
            <div className="ptl__identity">
              <span className={avatarClass} style={avatarStyle} aria-hidden="true">
                {avatarUrl ? <img ref={avatarRef} className="ptl__avatarimage" src={avatarUrl} alt=""
                  style={{ opacity: loadedAvatar === avatarUrl ? 1 : 0 }}
                  onLoad={() => setLoadedAvatar(avatarUrl)} onError={() => setFailedAvatar(avatarUrl)} /> : null}
                {loadedAvatar !== avatarUrl ? <span className="ptl__avatarinitial">{initial}</span> : null}
              </span>
              <div className="ptl__nameblock">
                <div className="ptl__namerow">
                  <h1 className="ptl__name">
                    {profile.name || truncateAddress(profile.address)}
                  </h1>
                  {hasClaimedName ? (
                    <span
                      className="ptl__verified"
                      style={nameColor ? { background: nameColor } : undefined}
                      title="Verified"
                    >
                      <VerifiedGlyph />
                    </span>
                  ) : (
                    <span className="ptl__discriminator">#{profile.address.slice(-4)}</span>
                  )}
                </div>
                <div className="ptl__addrrow">
                  {showWalletIcon ? <span className="ptl__walleticon"><WalletGlyph /></span> : null}
                  <span className="ptl__addr">{truncateAddress(profile.address)}</span>
                  {showCopy ? (
                    <button type="button" className="ptl__copy" aria-label="Copy address" onClick={onCopyAddress}>
                      <Copy size={16} />
                    </button>
                  ) : null}
                </div>
              </div>
            </div>

            {actions ? <div className="ptl__actions">{actions}</div> : null}
          </section>

          <ProfileNavigation tabs={tabs} active={activeTab} onTab={onTabChange} label={suffixLabel("Profile sections", labelSuffix)} />

          {aside ? (
            <div className="ptl__body ptl__body--split">
              <aside className="ptl__aside">{aside}</aside>
              <div className="ptl__main">{children}</div>
            </div>
          ) : (
            <div className={"ptl__body" + (bodyPadded ? " ptl__body--padded" : "")}>{children}</div>
          )}
        </div>
      </div>
    </div>
  );
}
