import type { MouseEvent as ReactMouseEvent } from "react";
import { Suspense, lazy, useState } from "react";
import { createPortal } from "react-dom";
import { Avatar } from "../../atoms/primitives";
import { profileFaceUrl, normalizeAddress } from "../../data/catalyst/profile";
import { useProfile } from "../../data/hooks/useProfile";
import { truncateAddress } from "../../data/format";
import ProfileCard from "./ProfileCard";
import "./person.css";

const loadPassport = () => import("../../app/panels/Passport.route");
const PassportPanel = lazy(loadPassport);

type Props = {
  address?: string | null;
  name?: string | null;
  size?: number;
  className?: string;
};

export default function Person({ address, name, size = 20, className = "" }: Props) {
  const addr = normalizeAddress(address) || "";
  const profile = useProfile(addr || null);
  const avatar = addr ? profile.data : null;
  const shown = avatar?.name?.trim() || name?.trim() || (addr ? truncateAddress(addr) : "");
  const face = avatar ? profileFaceUrl(avatar) : null;
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const [passport, setPassport] = useState(false);
  if (!addr) {
    return shown ? <span className={"person person--anon " + className}><Avatar size={size} name={shown} /><b>{shown}</b></span> : null;
  }
  return (
    <>
      <button
        type="button"
        className={"person " + className}
        onClick={(e: ReactMouseEvent) => { setAt({ x: e.clientX, y: e.clientY }); void loadPassport(); }}
        aria-haspopup="dialog"
        title={addr}
      >
        <Avatar size={size} name={shown} initials={/^0x/i.test(shown) ? addr.slice(2, 4).toUpperCase() : undefined} src={face ?? undefined} seed={addr} />
        <b>{shown}</b>
      </button>
      {at && <ProfileCard user={{ address: addr, name: shown, picture: face ?? undefined }} x={at.x} y={at.y} onClose={() => setAt(null)} onViewPassport={() => { setAt(null); setPassport(true); }} />}
      {passport && createPortal(<Suspense fallback={null}><PassportPanel address={addr} onClose={() => setPassport(false)} /></Suspense>, document.body)}
    </>
  );
}
