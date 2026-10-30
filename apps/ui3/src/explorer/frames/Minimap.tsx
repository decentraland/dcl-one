import { useState, useEffect, useMemo, useRef } from "react";
import type { ContextMenuItem } from "../../components/ContextMenu";
import { safeCssUrl } from "../../data/cssUrl";
import { siteUrl } from "../../data/site";
import { truncateAddress } from "../../data/format";
import { useFriendPins } from "../../data/hooks/useFriendPins";
import { useBridgeState } from "../../overlay/bridge";
import { useMinimapVisibility } from "../../overlay/minimapVisibility";
import ContextMenu from "../../components/ContextMenu";
import { SceneFeedbackModal, SceneTipModal, useSceneOwner } from "../components/SceneOwnerActions";
import RealmMap from "../pages/RealmMap";
import MapSurface, { GRID_PARCELS, useMapView } from "../components/MapSurface";
import { coordsToPercent } from "../../data/catalyst/places";
import type { FriendPin } from "../../data/hooks/useFriendPins";
import { realmMapBase } from "../../data/realmMap";
import "./minimap.css";
import MapCameraCone from "../components/MapCameraCone";
import Icon from "./SidebarDesignIcon";

const COORDS_RE = /^\s*(-?\d+)\s*,\s*(-?\d+)\s*$/;

function parseParcel(coords: string): { x: number; y: number } | null {
  if (typeof coords !== "string") return null;
  const m = coords.match(COORDS_RE);
  if (!m) return null;
  return { x: Number(m[1]), y: Number(m[2]) };
}

export function jumpUrl(coords: string, realm?: string | null): string {
  const p = parseParcel(coords);
  const pos = p ? `${p.x},${p.y}` : "0,0";
  const origin =
    typeof window !== "undefined" && window.location?.origin && window.location.origin !== "null"
      ? window.location.origin
      : siteUrl();
  return `${origin}/play/?position=${pos}${realm ? `&realm=${encodeURIComponent(realm)}` : ""}`;
}

const MINIMAP_PARCELS_ACROSS = 59;

function MinimapSurface({ parcel, friends }: { parcel: { x: number; y: number }; friends: FriendPin[] }) {
  const [view] = useMapView();
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState(236);
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof ResizeObserver === "undefined") return undefined;
    const measure = () => { const r = el.getBoundingClientRect(); if (r.width > 0) setSize(r.width); };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const zoom = GRID_PARCELS / MINIMAP_PARCELS_ACROSS;
  const here = coordsToPercent(`${parcel.x},${parcel.y}`);
  const panX = -(here.left / 100 - 0.5) * size * zoom;
  const panY = -(here.top / 100 - 0.5) * size * zoom;
  return (
    <div className="mm__surface" ref={ref} aria-hidden="true">
      <div
        className="mm__layer"
        style={{ width: size, height: size, transform: `translate(-50%, -50%) translate(${panX}px, ${panY}px) scale(${zoom})`, ["--inv" as string]: 1 / zoom }}
      >
        <MapSurface view={view} zoom={zoom} viewport={{ w: size, h: size, square: size, panX, panY }} />
        {friends.map((f) => {
          const pos = coordsToPercent(`${f.x},${f.y}`);
          const bg = safeCssUrl(f.picture);
          return <span key={f.address} className="mm__friend" style={{ left: `${pos.left}%`, top: `${pos.top}%`, ...(bg ? { backgroundImage: bg } : null) }} title={f.name} />;
        })}
      </div>
    </div>
  );
}

function MenuLabel({ main, hint }: { main: string; hint: string }) {
  return (
    <span className="mm__mlabel">
      <span className="mm__mmain">{main}</span>
      <span className="mm__mhint">{hint}</span>
    </span>
  );
}

function ViewToggle() {
  const [view, setView] = useMapView();
  const items = [
    { key: "map" as const, label: "Map", icon: (
      <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true">
        <polygon points="9 3 3 5.7 3 20.3 9 17.6 15 20.3 21 17.6 21 3 15 5.7 9 3" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" />
        <path d="M9 3v14.6M15 5.7v14.6" fill="none" stroke="currentColor" strokeWidth="2" />
      </svg>
    ) },
    { key: "satellite" as const, label: "Satellite", icon: (
      <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true">
        <g transform="rotate(45 12 12)" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round">
          <rect x="9.5" y="9.5" width="5" height="5" rx="1" />
          <rect x="1.5" y="10.2" width="6" height="3.6" />
          <rect x="16.5" y="10.2" width="6" height="3.6" />
        </g>
      </svg>
    ) },
  ];
  return (
    <div className="mm__viewswitch" role="radiogroup" aria-label="Minimap view">
      {items.map((it) => (
        <button
          key={it.key}
          type="button"
          role="radio"
          aria-checked={view === it.key}
          aria-label={it.label}
          title={it.label}
          className={"mm__viewbtn" + (view === it.key ? " is-on" : "")}
          onClick={() => setView(it.key)}
        >
          {it.icon}
        </button>
      ))}
    </div>
  );
}

type OwnerModal = "feedback" | "tip" | null;

type MinimapProps = {
  place?: string;
  coords?: string;
  heading?: number;
  cameraHeading?: number;
  inline?: boolean;
  alwaysShown?: boolean;
};

export default function Minimap({ place = "", coords = "", heading, cameraHeading, inline = false, alwaysShown = false }: MinimapProps) {

  const [menuOpen, setMenuOpen] = useState(false);
  const [ownerModal, setOwnerModal] = useState<OwnerModal>(null);
  const kebabRef = useRef<HTMLDivElement>(null);
  const { minimapHidden, userHidden, toggleUserHidden } = useMinimapVisibility();
  const realm = useBridgeState((s) => s.scene.realm);
  const realmBase = useMemo(() => realmMapBase(typeof window === "undefined" ? "" : window.location.search, realm), [realm]);
  const shown = !(minimapHidden || userHidden);
  const owner = useSceneOwner(coords, realm, (menuOpen || ownerModal !== null) && shown);
  const parcel = parseParcel(coords);
  const parcelText = parcel ? `${parcel.x},${parcel.y}` : "";
  const friendPins = useFriendPins();

  useEffect(() => {
    if (!menuOpen) return;
    const onDown = (e: PointerEvent) => {
      const target = e.target;
      if (kebabRef.current && target instanceof Node && !kebabRef.current.contains(target)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => {
      document.removeEventListener("pointerdown", onDown, true);
    };
  }, [menuOpen]);

  function copyText(text: string) {
    if (!text) return;
    try {
      navigator.clipboard?.writeText(text);
    } catch {
    }
  }

  function pick(action: () => void) {
    setMenuOpen(false);
    action();
  }

  const ownerHint = (owner.tipAddress || owner.address)
    ? `recipient ${truncateAddress((owner.tipAddress || owner.address)!)}`
    : owner.loading
      ? "Finding recipient\u{2026}"
      : owner.world
        ? "World tip recipient unavailable"
        : "Tip recipient unavailable";

  const menuItems: ContextMenuItem[] = [
    {
      kind: "button",
      label: <MenuLabel main="Copy coordinates" hint={parcelText || "no parcel"} />,
      disabled: !parcel,
      onClick: () => pick(() => copyText(parcelText)),
    },
    {
      kind: "button",
      label: <MenuLabel main="Copy jump link" hint={jumpUrl(coords)} />,
      disabled: !parcel,
      onClick: () => pick(() => copyText(jumpUrl(coords))),
    },
    { kind: "separator" },
    {
      kind: "button",
      label: <MenuLabel main="Send feedback" hint="Choose a scene recipient" />,
      onClick: () => pick(() => setOwnerModal("feedback")),
    },
    {
      kind: "button",
      icon: <Icon name="heart" />,
      label: <MenuLabel main="Send tip" hint={ownerHint} />,
      disabled: !(owner.tipAddress || owner.address),
      onClick: () => pick(() => setOwnerModal("tip")),
    },
  ];

  if (!shown && !alwaysShown) return null;

  const modalScene = place || owner.sceneTitle || "";

  return (
    <div className={"mm__stage" + (inline ? " mm__stage--inline" : "")}>
      <div className="mm">
        {!inline && <div className="mm__header">
          <div className="mm__place">
            <span className="mm__name u-truncate">{place}</span>
            <span className="mm__coords">
              <svg className="mm__pin" viewBox="0 0 24 24" width="11" height="11" aria-hidden="true">
                <path d="M12 2c-3.9 0-7 3-7 6.9 0 4.6 7 12.1 7 12.1s7-7.5 7-12.1C19 5 15.9 2 12 2z"
                  fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" />
                <circle cx="12" cy="9" r="2.4" fill="currentColor" />
              </svg>
              {coords}
            </span>
          </div>

          <div className="mm__actions">
            <ViewToggle />
            <button
              type="button"
              className="mm__hide"
              onClick={toggleUserHidden}
              aria-label="Hide map"
              title="Hide map"
            >
              <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
                <path d="M5 12h14" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" />
              </svg>
            </button>
            <div className="mm__kebab-wrap" ref={kebabRef}>
              <button
                className="mm__kebab"
                title="Scene options"
                aria-label="Scene options"
                aria-haspopup="menu"
                aria-expanded={menuOpen}
                onClick={() => setMenuOpen((o) => !o)}
              >
                <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
                  <circle cx="12" cy="5" r="1.7" fill="currentColor" />
                  <circle cx="12" cy="12" r="1.7" fill="currentColor" />
                  <circle cx="12" cy="19" r="1.7" fill="currentColor" />
                </svg>
              </button>

              {menuOpen && (
                <div className="mm__menu">
                  <ContextMenu items={menuItems} onClose={() => setMenuOpen(false)} autoFocus />
                </div>
              )}
            </div>
          </div>
        </div>}

        <button
          type="button"
          className="mm__map"
          data-sb-linkto="Explorer/Pages/Map"
          aria-label="Open full map"
          title="Open map"
        >
            {parcel && (realmBase
              ? <RealmMap base={realmBase} coords={parcelText} heading={heading} variant="mini" />
              : <MinimapSurface parcel={parcel} friends={friendPins} />)}
            <span className="mm__compass mm__compass--n" aria-hidden="true">N</span>
            <span className="mm__compass mm__compass--e" aria-hidden="true">E</span>
            <span className="mm__compass mm__compass--s" aria-hidden="true">S</span>
            <span className="mm__compass mm__compass--w" aria-hidden="true">W</span>
            <MapCameraCone heading={cameraHeading} />
            <svg
              className="mm__player"
              viewBox="0 0 24 24"
              width="22"
              height="22"
              aria-hidden="true"
              style={heading == null ? undefined : { transform: `translate(-50%, -50%) rotate(${heading}deg)` }}
            >
              <path d="M12 3l7 16-7-4-7 4 7-16z" fill="var(--brand)"
                stroke="#fff" strokeWidth="1.6" strokeLinejoin="round" />
            </svg>
          </button>
      </div>

      {ownerModal === "feedback" && (
        <SceneFeedbackModal
          owner={owner.address}
          recipients={owner.recipients}
          loading={owner.loading}
          sceneTitle={modalScene}
          coords={parcelText || coords}
          onClose={() => setOwnerModal(null)}
        />
      )}
      {ownerModal === "tip" && (owner.tipAddress || owner.address) && (
        <SceneTipModal
          recipients={owner.recipients}
          loading={owner.loading}
          owner={(owner.tipAddress || owner.address)!}
          sceneTitle={modalScene}
          coords={parcelText || coords}
          onClose={() => setOwnerModal(null)}
        />
      )}
    </div>
  );
}
