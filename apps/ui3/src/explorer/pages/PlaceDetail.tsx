import type { CSSProperties, ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { textClasses } from "@dcl/one-ui/components";
// The Decentraland logo with its two suns drawn as arcs, as decentraland.org draws the avatar.
import foundationLogo from "./foundation.svg?url";
import { sameOriginContentUrl } from "../../data/catalyst/profile";
import { asAddress } from "../../data/catalyst/sceneOwner";
import { fetchCommunityProfiles } from "../../data/hooks/useCommunityProfiles";
import { STALE } from "../../data/queryKeys";
import { getAvatarBackgroundColor, getDisplayName } from "../../shop/discovery/lib/avatarColor";
import styles from "./PlaceDetail.module.css";

// from: sites@17b3535a:src/pages/places/DiscoverScenePage.tsx, src/components/places/SceneLiveWatcher,
// src/components/places/SceneJumpInModal, src/components/cast/ChatPanel, src/hooks/usePlaceCreator.ts

export type PlaceDetailData = {
  id: string;
  title: string;
  description: string;
  image?: string | null;
  coords: string;
  world?: boolean;
  worldName?: string | null;
  contactName?: string | null;
  owner?: string | null;
  creatorAddress?: string | null;
};

type PlaceDetailProps = {
  place?: PlaceDetailData;
  notFound?: boolean;
  /** Players in the scene now; with none (or on a phone) the page is the jump-in card. */
  livePlayers?: number;
  mobile?: boolean;
  jumpHref?: string;
  onJumpIn?: () => void;
  onClose?: () => void;
  /** Opens the creator's profile; without it the creator's name is plain text. */
  onCreator?: (address: string) => void;
  detailSlot?: ReactNode;
  /** A world's scenes: with more than one, the live view's info panel picks the one to jump into. */
  scenePicker?: ScenePickerProps;
  /** The live view's In-World Chat; until the host knows the room it is loading. */
  chat?: SceneChat;
};

/** One chat message heard in the scene's room; `sender` is a wallet when the room knew one, else a display name. */
export type SceneChatMessage = { sender: string; timestamp: number; message: string };
/** `loading` until the room is known, `ready` once it is (connecting or connected), `unavailable` when there is none. */
export type SceneChat = { status: "loading" | "ready" | "unavailable"; messages: SceneChatMessage[] };
const CHAT_LOADING: SceneChat = { status: "loading", messages: [] };

const FOUNDATION = "Decentraland Foundation";
const DEFAULT_THUMBNAILS = new Set([
  "bafkreidj26s7aenyxfthfdibnqonzqm5ptc4iamml744gmcyuokewkr76y",
  "bafkreiccptidkodtsbcmk7uw3afmuysu6eboocnr57dmwdg7voylyzg5hy",
]);

function coverImage(image?: string | null): string | undefined {
  return !image || image.includes("/v2/map.png") || DEFAULT_THUMBNAILS.has(image.split("/").pop() ?? "") ? undefined : image;
}

function syntheticAvatar(name: string): string {
  const initial = (name.match(/[\p{L}\p{N}]/u)?.[0] ?? "?").toUpperCase();
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" fill="${getAvatarBackgroundColor(name)}"/><text x="16" y="22" text-anchor="middle" font-family="sans-serif" font-size="18" font-weight="700" fill="white">${initial}</text></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

type Creator = { name?: string; avatar?: string; background?: string; address?: string };

function usePlaceCreator(place: PlaceDetailData | undefined): Creator {
  const contact = place?.contactName?.trim();
  const contactName = contact && contact.toLowerCase() !== "sdk" ? contact : undefined;
  const foundation = contactName?.toLowerCase() === FOUNDATION.toLowerCase();
  const address = contactName && !foundation
    ? asAddress(place?.creatorAddress) ?? asAddress(place?.owner) ?? undefined
    : undefined;
  const profiles = useQuery({
    queryKey: ["place-creator-profile", address],
    queryFn: ({ signal }) => fetchCommunityProfiles([address as string], { signal }),
    enabled: Boolean(address),
    staleTime: STALE.profile,
    retry: false,
  }).data;
  const profile = address ? profiles?.[address] : undefined;
  const name = foundation ? FOUNDATION : contactName || place?.title?.trim() || undefined;
  if (!name) return {};
  if (foundation) return { name, avatar: foundationLogo, background: "#9d76e3" };
  const face = profile?.picture ? sameOriginContentUrl(profile.picture) : undefined;
  const background = getAvatarBackgroundColor(address ? getDisplayName(profile?.name || null, profile?.claimed, address) : name);
  return { name, avatar: face || syntheticAvatar(name), background, address };
}

function PinGlyph() {
  return (
    <svg className={styles.pin} viewBox="0 0 15 15" fill="none" aria-hidden="true">
      <path fillRule="evenodd" clipRule="evenodd" fill="currentColor" d="M11.082 10.394C10.3068 11.565 9.41232 12.6461 8.90834 13.2307C8.21939 14.0298 6.99075 14.0298 6.3018 13.2307C5.79782 12.6461 4.90333 11.565 4.12816 10.394C3.74091 9.80902 3.36728 9.1775 3.08668 8.55678C2.81567 7.95727 2.58267 7.26354 2.58267 6.58317C2.58267 3.79361 4.81361 1.5 7.60507 1.5C10.3965 1.5 12.6275 3.79361 12.6275 6.58317C12.6275 7.26354 12.3945 7.95727 12.1235 8.55678C11.8429 9.1775 11.4692 9.80902 11.082 10.394ZM8.00374 12.4508C7.79124 12.6973 7.4189 12.6973 7.2064 12.4508C6.22605 11.3137 3.77702 8.30103 3.77702 6.58317C3.77702 4.43544 5.4909 2.69436 7.60507 2.69436C9.71924 2.69436 11.4331 4.43544 11.4331 6.58317C11.4331 8.30103 8.98409 11.3137 8.00374 12.4508Z" />
      <path fillRule="evenodd" clipRule="evenodd" fill="currentColor" d="M7.6043 7.95758C8.35176 7.95758 8.9577 7.34202 8.9577 6.58269C8.9577 5.82336 8.35176 5.20781 7.6043 5.20781C6.85683 5.20781 6.25089 5.82336 6.25089 6.58269C6.25089 7.34202 6.85683 7.95758 7.6043 7.95758Z" />
    </svg>
  );
}

function JumpInGlyph({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24.8735 24.8735" fill="none" aria-hidden="true">
      <rect className={styles.glyphFill} width="24.8735" height="24.8735" rx="8.29118" />
      <rect x="0.777298" y="0.777298" width="23.3189" height="23.3189" rx="7.51388" className={styles.glyphFrame} strokeOpacity="0.3" strokeWidth="1.5546" />
      <path className={styles.glyphArrow} d="M18.9554 11.031L14.1081 6.1861C13.244 5.32244 11.7898 5.93332 11.7898 7.15508V8.75601C11.7476 8.75601 11.7265 8.75601 11.6844 8.75601H7.08641C6.30662 8.75601 5.67436 9.36689 5.67436 10.1463V13.8326C5.67436 14.612 6.30662 15.244 7.08641 15.244H11.6633C11.7055 15.244 11.7265 15.244 11.7687 15.244V16.8449C11.7687 18.0667 13.244 18.6776 14.087 17.8139L18.9343 12.969C19.4823 12.4213 19.4823 11.5576 18.9554 11.031Z" />
    </svg>
  );
}

function Avatar({ creator }: { creator: Creator }) {
  if (!creator.avatar) return null;
  return <img className={styles.avatar} src={creator.avatar} alt="" style={{ "--avatar-bg": creator.background } as CSSProperties} />;
}

function JumpIn({ href, onClick, className, glyph }: { href?: string; onClick?: () => void; className?: string; glyph?: string }) {
  const body = <>Jump in<JumpInGlyph className={glyph} /></>;
  return href
    ? <a className={className} href={href} onClick={onClick}>{body}</a>
    : <button type="button" className={className} onClick={onClick}>{body}</button>;
}

const coverStyle = (cover?: string) => (cover ? ({ "--cover": `url("${cover}")` } as CSSProperties) : undefined);

function SceneJumpIn({ place, creator, livePlayers, jumpHref, onJumpIn, onClose }: {
  place: PlaceDetailData; creator: Creator; livePlayers: number; jumpHref?: string; onJumpIn?: () => void; onClose?: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => { if (timer.current) clearTimeout(timer.current); }, []);
  useEffect(() => {
    if (!onClose) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);
  const copyLink = () => {
    navigator.clipboard?.writeText(window.location.href).then(() => {
      setCopied(true);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 2000);
    }).catch(() => undefined);
  };
  const location = place.world ? place.worldName || undefined : place.coords || undefined;
  return (
    <div className={styles.backdrop} onClick={(e) => { if (e.target === e.currentTarget) onClose?.(); }}>
      <div className={styles.modal} role="dialog" aria-modal="true" aria-label={place.title}>
        <button type="button" className={styles.close} aria-label="Close" onClick={onClose}>
          <svg className={styles.closeGlyph} viewBox="0 0 40 40" fill="none" aria-hidden="true">
            <path d="M25.8828 14.1174L14.1184 25.8818" className={styles.closeStroke} strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
            <path d="M25.8828 25.8826L14.1184 14.1182" className={styles.closeStroke} strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>
        <div className={styles.heroWrap}>
          <div className={styles.hero} style={coverStyle(coverImage(place.image))}>
            <div className={styles.heroFade} />
            {livePlayers > 0 && (
              <div className={styles.heroBadges}>
                <span className={styles.liveBadge}>
                  <svg className={styles.badgeIcon} viewBox="0 0 24 24" aria-hidden="true" data-testid="SensorsRoundedIcon">
                    <path d="M8.54 8.54c.35.35.37.88.1 1.29C8.24 10.45 8 11.2 8 12c0 .8.24 1.55.64 2.17.27.41.24.95-.11 1.29-.43.43-1.17.4-1.51-.11C6.38 14.4 6 13.24 6 12c0-1.21.36-2.33.97-3.28.36-.54 1.11-.64 1.57-.18m6.92 6.92c.43.43 1.17.4 1.51-.11C17.62 14.4 18 13.24 18 12c0-1.24-.38-2.4-1.03-3.36-.34-.5-1.08-.54-1.51-.11-.35.35-.37.88-.11 1.29.41.63.65 1.38.65 2.18 0 .8-.24 1.55-.64 2.17-.27.41-.24.95.1 1.29M12 10c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2m6.32 8.32c.42.42 1.12.39 1.5-.08C21.18 16.53 22 14.36 22 12s-.82-4.53-2.18-6.24c-.37-.47-1.07-.5-1.5-.08-.36.36-.4.92-.08 1.32 1.1 1.37 1.76 3.11 1.76 5s-.66 3.63-1.76 5c-.32.39-.28.96.08 1.32M5.68 5.68c-.42-.42-1.12-.39-1.5.08C2.82 7.47 2 9.64 2 12s.82 4.53 2.18 6.24c.37.47 1.07.5 1.5.08.36-.36.4-.92.08-1.32C4.66 15.63 4 13.89 4 12s.66-3.63 1.76-5c.32-.39.28-.96-.08-1.32" />
                  </svg>
                  Live
                </span>
                <span className={styles.countBadge}>
                  <span className={styles.countDot} />
                  <svg className={styles.badgeIcon} viewBox="0 0 24 24" aria-hidden="true" data-testid="PermIdentityRoundedIcon">
                    <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4m0-6c1.1 0 2 .9 2 2s-.9 2-2 2-2-.9-2-2 .9-2 2-2m0 7c-2.67 0-8 1.34-8 4v2c0 .55.45 1 1 1h14c.55 0 1-.45 1-1v-2c0-2.66-5.33-4-8-4m6 5H6v-.99c.2-.72 3.3-2.01 6-2.01s5.8 1.29 6 2z" />
                  </svg>
                  {livePlayers}
                </span>
              </div>
            )}
          </div>
          <div className={styles.heroText}>
            <h1 className={styles.modalTitle}>{place.title}</h1>
            <div className={styles.metaRow}>
              {creator.name && (
                <div className={styles.creatorRow}>
                  <Avatar creator={creator} />
                  <p className={styles.modalBy}>By{" "}<span className={styles.modalCreatorName}>{creator.name}</span></p>
                </div>
              )}
              {location && <div className={styles.modalLocation}><PinGlyph />{location}</div>}
            </div>
            <div className={styles.ctaRow}>
              <JumpIn href={jumpHref} onClick={onJumpIn} className={styles.modalJump} glyph={styles.modalJumpGlyph} />
              <button type="button" className={styles.copy} aria-label="Copy link" onClick={copyLink}>
                <svg className={styles.copyGlyph} viewBox="0 0 20 20" fill="none" aria-hidden="true">
                  <path d="M15.3854 15H16C17.6569 15 19 13.6569 19 12V4C19 2.34315 17.6569 1 16 1H8C6.34315 1 5 2.34315 5 4V4.88124" className={styles.copyStroke} strokeWidth="2" strokeLinejoin="round" />
                  <rect x="1" y="5" width="14" height="14" rx="3" className={styles.copyStroke} strokeWidth="2" strokeLinejoin="round" />
                </svg>
                {copied && <span className={styles.copied} role="status">Copied!</span>}
              </button>
            </div>
          </div>
        </div>
        {place.description && (
          <div className={styles.about}>
            <p className={styles.infoLabel}>What to expect</p>
            <p className={styles.infoText}>{place.description}</p>
          </div>
        )}
      </div>
    </div>
  );
}

const formatTime = (timestamp: number) => new Date(timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

/**
 * The senders' profiles, for the wallets among them: a claimed name, else name#tag, else the short address.
 * Every wallet asked for is in the answer, with or without a profile, so one without stops waiting.
 */
function useSenderProfiles(messages: SceneChatMessage[]) {
  const addresses = [...new Set(messages.map((m) => asAddress(m.sender)).filter((a): a is string => Boolean(a)))].sort();
  return useQuery({
    queryKey: ["scene-chat-profiles", addresses],
    queryFn: async ({ signal }) => {
      const found = await fetchCommunityProfiles(addresses, { signal });
      return Object.fromEntries(addresses.map((address) => [address, found[address] ?? { name: "", picture: null, claimed: null }]));
    },
    enabled: addresses.length > 0,
    staleTime: STALE.profile,
    retry: false,
  }).data;
}

function ChatMessage({ message, profiles }: { message: SceneChatMessage; profiles: Record<string, { name: string; picture: string | null; claimed: boolean | null }> | undefined }) {
  const address = asAddress(message.sender);
  const profile = address ? profiles?.[address] : undefined;
  let name = message.sender || "Anonymous";
  if (address) {
    const fromProfile = getDisplayName(profile?.name || null, profile?.claimed, address);
    // Until the profile arrives the name is a placeholder, so it never flips from the address to the name.
    name = fromProfile || (profiles && address in profiles ? `${address.slice(0, 6)}...${address.slice(-4)}` : "…");
  }
  const face = profile?.picture ? sameOriginContentUrl(profile.picture) : undefined;
  const avatar = face || syntheticAvatar(name === "…" ? address ?? name : name);
  return (
    <div className={styles.message}>
      <div className={styles.messageHeader}>
        <img className={styles.messageAvatar} src={avatar} alt="" style={{ "--avatar-bg": getAvatarBackgroundColor(name) } as CSSProperties} />
        <span className={styles.messageName}>{name}</span>
        <span className={styles.messageTime}>{formatTime(message.timestamp)}</span>
      </div>
      <div className={styles.messageText}>{message.message}</div>
    </div>
  );
}

/**
 * The In-World Chat column: always the same shell (header, messages, jump-in footer); while the room is being
 * asked for it shows the empty state, and when there is no room it says so. Read-only: viewers take part from the client.
 */
function SceneChatPanel({ chat, title, jumpHref }: { chat: SceneChat; title: string; jumpHref?: string }) {
  const profiles = useSenderProfiles(chat.messages);
  const endRef = useRef<HTMLDivElement>(null);
  const count = chat.messages.length;
  useEffect(() => {
    endRef.current?.scrollIntoView?.({ behavior: "smooth", block: "nearest", inline: "nearest" });
  }, [count]);
  const empty = chat.status !== "ready" || count === 0;
  return (
    <div className={styles.chat}>
      <div className={styles.chatHeader}>
        <h2 className={styles.chatTitle}>In-World Chat</h2>
      </div>
      <div className={empty ? styles.chatMessages : `${styles.chatMessages} ${styles.chatMessagesList}`}>
        {empty
          ? <p className={`${textClasses.bodyMd} ${styles.chatEmpty}`}>{chat.status === "unavailable" ? "Chat is unavailable right now." : "No messages yet."}</p>
          : <>{chat.messages.map((m, i) => <ChatMessage key={i} message={m} profiles={profiles} />)}<div ref={endRef} /></>}
      </div>
      <p className={styles.chatFooter}>
        Jump into{" "}
        {jumpHref
          ? <a className={styles.chatLink} href={jumpHref} target="_blank" rel="noopener noreferrer">{title}</a>
          : title}{" "}
        in Decentraland to participate in the chat.
      </p>
    </div>
  );
}

function SceneWatcher({ place, jumpHref, onJumpIn }: { place: PlaceDetailData; jumpHref?: string; onJumpIn?: () => void }) {
  const [launched, setLaunched] = useState(false);
  return (
    <div className={styles.watcher}>
      <div className={styles.video}>
        {launched && jumpHref
          ? <iframe className={styles.frame} src={jumpHref} title={place.title} allow="fullscreen; clipboard-read; clipboard-write" />
          : <div className={styles.launch} style={coverStyle(coverImage(place.image))} />}
      </div>
      <div className={styles.controls}>
        <div className={styles.controlButtons}>
          {launched
            ? <button type="button" className={styles.explore} onClick={() => setLaunched(false)}>Stop</button>
            : <button type="button" className={styles.explore} disabled={!jumpHref} onClick={() => setLaunched(true)}>Explore the scene</button>}
          <JumpIn href={jumpHref} onClick={onJumpIn} className={styles.jump} glyph={styles.jumpGlyph} />
        </div>
      </div>
    </div>
  );
}

export type ScenePickerProps = { options: { value: string; label: string }[]; value: string; onChange: (value: string) => void };

function ScenePicker({ options, value, onChange }: ScenePickerProps) {
  return (
    <label className={styles.pickerRow}>
      <span className={styles.pickerLabel}>Scene</span>
      <select className={styles.pickerSelect} value={value} onChange={(e) => onChange(e.target.value)}>
        {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
      </select>
    </label>
  );
}

export default function PlaceDetail({ place, notFound = false, livePlayers = 0, mobile = false, jumpHref, onJumpIn, onClose, onCreator, detailSlot, scenePicker, chat = CHAT_LOADING }: PlaceDetailProps) {
  const creator = usePlaceCreator(place);
  if (notFound || !place) {
    return (
      <div className={styles.page}>
        <div className={styles.notFound}>
          <h1 className={styles.title}>Scene not found</h1>
          <p className={styles.notFoundHint}>We couldn&apos;t load that place or world. It may have been removed.</p>
        </div>
      </div>
    );
  }
  if (mobile || livePlayers === 0) {
    return (
      <div className={styles.page}>
        <SceneJumpIn place={place} creator={creator} livePlayers={livePlayers} jumpHref={jumpHref} onJumpIn={onJumpIn} onClose={onClose} />
        {detailSlot}
      </div>
    );
  }
  const location = place.world ? place.worldName || "" : place.coords;
  const showPicker = scenePicker !== undefined && scenePicker.options.length > 1;
  return (
    <div className={styles.page}>
      <div className={styles.content}>
        <section className={styles.viewer} aria-label={place.title}>
          <div className={styles.viewerHeader}>
            <h1 className={styles.title}>{place.title}</h1>
            <div className={styles.headerRight}>
              {creator.name && (
                <div className={styles.creatorRow}>
                  <Avatar creator={creator} />
                  <p className={styles.by}>
                    By{" "}
                    {creator.address && onCreator
                      ? <button type="button" className={styles.creatorName} onClick={() => onCreator(creator.address!)}>{creator.name}</button>
                      : <span className={styles.creatorName}>{creator.name}</span>}
                  </p>
                </div>
              )}
              {location && (
                <a className={styles.location} href={jumpHref} target="_blank" rel="noopener noreferrer"
                  aria-label={jumpHref ? `Jump into ${location} (opens in a new tab)` : undefined}>
                  <PinGlyph />
                  {location}
                </a>
              )}
            </div>
          </div>
          <SceneWatcher place={place} jumpHref={jumpHref} onJumpIn={onJumpIn} />
        </section>

        <div className={styles.chatColumn}>
          <SceneChatPanel chat={chat} title={place.title} jumpHref={jumpHref} />
        </div>

        {(place.description || detailSlot || showPicker) && (
          <div className={styles.info}>
            {detailSlot}
            {showPicker && <ScenePicker {...scenePicker} />}
            {place.description && (
              <>
                <p className={styles.infoLabel}>What to expect</p>
                <p className={styles.infoText}>{place.description}</p>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
