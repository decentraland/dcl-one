
import type { ButtonHTMLAttributes, ComponentProps, KeyboardEvent as ReactKeyboardEvent, MouseEvent as ReactMouseEvent, ReactNode, SetStateAction } from "react";
import { Fragment, useContext, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { QueryClientContext } from "@tanstack/react-query";
import type { NearbyPlayer } from "../../generated/bridge/NearbyPlayer";
import { Avatar } from "../../atoms/primitives";
import logo from "../../atoms/assets/decentraland-logo.svg?url";
import type { BridgeChatLine } from "../../overlay/bridge";
import { EmojiPicker } from "./EmojiPicker";
import { getEmojiData, loadEmojiData, searchByShortcode, SHORTCODE_RE, type Emoji } from "./emojiData";
import { MessageText, mentionsMe, buildNameIndex } from "./chatText";
import { consoleLineKey, dispatchCommand, helpText, requestEngineHelp, wallClockMs, type ConsoleSource } from "./chatCommands";
import type { ProfileCardProps } from "../components/ProfileCard";
import type { ProfileCardUser } from "../components/ProfileCardPresentation";
import styles from "./Chat.module.css";
import { truncateIfAddress } from "../../data/format";
import { sendFriendRequest } from "../../data/hooks/friendActions";
import { useProfile } from "../../data/hooks/useProfile";
import { profileFaceUrl } from "../../data/catalyst/profile";

export type ChatIo = {
  chat: BridgeChatLine[];
  players: NearbyPlayer[];
  me: { address: string; name: string } | null;
  blocked: string[];
  live: boolean;
  send: (message: string) => void | Promise<void>;
  console?: ConsoleSource;
  teleport?: (x: number, z: number) => void;
  changeRealm?: (realm: string) => void;
};

const MAX_LEN = 500;
const PARCEL_SIZE = 16;

const RARITY = [
  "#73d3d3", "#acf8f8", "#ff8362", "#ff4bed", "#caff73", "#a14bf3",
  "#e8b9ff", "#fea217", "#81e1ff", "#ff7439", "#ffa25a", "#ffc95b",
  "#a0abff", "#c640cd",
];
const SYSTEM_COLOR = "#61d04f";
const CONSOLE_CHANNEL = "System";

function isSystem(sender: string): boolean {
  return !sender || sender.toLowerCase() === "system";
}

function displaySender(sender: string): string {
  if (isSystem(sender)) return "DCL System";
  return truncateIfAddress(sender);
}

function memberLabel(m: NearbyPlayer): string {
  return m.name.trim() ? m.name : truncateIfAddress(m.address);
}

function findMember(members: NearbyPlayer[], address: string): NearbyPlayer | undefined {
  return members.find((m) => m.address.toLowerCase() === address.toLowerCase());
}

function splitName(label: string): { base: string; tag: string } {
  const i = label.indexOf("#");
  return i >= 0 ? { base: label.slice(0, i), tag: label.slice(i) } : { base: label, tag: "" };
}

function initialOf(name: string): string {
  return name.replace(/^[@#]/, "").match(/[a-z0-9]/i)?.[0] ?? name.slice(0, 1);
}

function hash(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0;
  return h;
}

function senderColor(sender: string): string {
  if (isSystem(sender)) return SYSTEM_COLOR;
  return RARITY[hash(sender) % RARITY.length] ?? SYSTEM_COLOR;
}

function hexToHue(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const r = ((n >> 16) & 255) / 255;
  const g = ((n >> 8) & 255) / 255;
  const b = (n & 255) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  if (max === min) return 0;
  const d = max - min;
  let h: number;
  if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return Math.round(h * 60);
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

function dayKey(ts: number): string {
  const d = new Date(ts);
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
}

function formatDay(ts: number): string {
  if (dayKey(ts) === dayKey(Date.now())) return "Today";
  return new Date(ts).toLocaleDateString([], {
    weekday: "short",
    day: "numeric",
    month: "short",
  });
}

function PersonIcon() {
  return (
    <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="5" r="2.6" stroke="currentColor" strokeWidth="1.4" />
      <path
        d="M3.2 13c0-2.4 2.1-3.8 4.8-3.8s4.8 1.4 4.8 3.8"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
    </svg>
  );
}

function Smiley() {
  return (
    <svg width="20" height="20" viewBox="0 0 22 22" fill="none" aria-hidden="true">
      <circle cx="11" cy="11" r="10" stroke="currentColor" strokeWidth="2" />
      <circle cx="7.6" cy="9" r="1.15" fill="currentColor" />
      <circle cx="14.4" cy="9" r="1.15" fill="currentColor" />
      <path
        d="M7 13.4c1 1.6 2.5 2.4 4 2.4s3-.8 4-2.4"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
      />
    </svg>
  );
}

function CharRing({ len }: { len: number }) {
  const pct = Math.min(1, len / MAX_LEN);
  const r = 8;
  const circ = 2 * Math.PI * r;
  const color = pct >= 0.9 ? "var(--brand)" : pct >= 0.7 ? "var(--gold)" : "var(--green)";
  return (
    <svg className={styles.ring} width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
      <circle cx="10" cy="10" r={r} fill="none" stroke="rgba(255,255,255,0.25)" strokeWidth="2" />
      <circle
        cx="10"
        cy="10"
        r={r}
        fill="none"
        stroke={color}
        strokeWidth="2"
        strokeLinecap="round"
        strokeDasharray={circ}
        strokeDashoffset={circ * (1 - pct)}
        transform="rotate(-90 10 10)"
      />
    </svg>
  );
}

function CtrlButton({
  variant = "ghost",
  shape = "square",
  size = "md",
  active = false,
  className = "",
  ...rest
}: {
  variant?: "ghost" | "solid";
  shape?: "square" | "circle" | "pill";
  size?: "sm" | "md";
  active?: boolean;
} & ButtonHTMLAttributes<HTMLButtonElement>) {
  const variantClass = variant === "solid" ? styles.ctrlSolid : styles.ctrlGhost;
  const shapeClass = shape === "circle" ? styles.ctrlCircle : shape === "pill" ? styles.ctrlPill : styles.ctrlSquare;
  const sizeClass = size === "sm" ? styles.ctrlSm : styles.ctrlMd;
  return (
    <button
      type="button"
      className={`${styles.ctrlBtn} ${variantClass} ${shapeClass} ${sizeClass} ${active ? styles.ctrlActive : ""} ${className}`.trim()}
      aria-pressed={active}
      {...rest}
    />
  );
}

export const FRESH_MS = 10_000;

export function useNow(stamps: readonly number[], windowMs = FRESH_MS): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = Date.now();
    if (stamps.some((ts) => now - ts < windowMs && t - ts >= windowMs)) {
      setNow(t);
      return undefined;
    }
    let next = Infinity;
    for (const ts of stamps) if (ts + windowMs > t) next = Math.min(next, ts + windowMs);
    if (next === Infinity) return undefined;
    const id = setTimeout(() => setNow(Date.now()), next - t);
    return () => clearTimeout(id);
  }, [stamps, windowMs, now]);
  return now;
}

const WALLET = /^0x[0-9a-f]{40}$/i;

function ProfileFace({ address, ...props }: ComponentProps<typeof Avatar> & { address: string }) {
  const profile = useProfile(address);
  const [failed, setFailed] = useState(0);
  const raw = profile.data?.avatar?.snapshots?.face256;
  const faces = [profile.data ? profileFaceUrl(profile.data) : null, typeof raw === "string" && /^https:\/\//i.test(raw) ? raw : null]
    .filter((url, i, all): url is string => !!url && all.indexOf(url) === i);
  return <Avatar {...props} src={faces[failed]} onImageError={() => setFailed((n) => n + 1)} />;
}

export function SenderAvatar({ address, src, ...props }: ComponentProps<typeof Avatar> & { address: string }) {
  const client = useContext(QueryClientContext);
  return !src && client && WALLET.test(address) ? <ProfileFace address={address} {...props} /> : <Avatar src={src} {...props} />;
}

function DaySeparator({ ts, stale = false }: { ts: number; stale?: boolean }) {
  return (
    <div className={styles.dayRow} data-chat-stale={stale || undefined}>
      <span className={styles.dayPill}>{formatDay(ts)}</span>
    </div>
  );
}

const MSG_STYLES = { url: styles.url, mention: styles.mention, location: styles.location, world: styles.world };

export type ChatLine = { id: string; ts: number; sender: string; senderName?: string; message: string; channel?: string };
type LocalLine = { id: string; ts: number; message: string; anchor: BridgeChatLine | null; anchorKey?: string };

const receivedAt = new WeakMap<BridgeChatLine, number>();
let localSeq = 0;

export function stampOf(l: BridgeChatLine): number {
  const seen = receivedAt.get(l);
  if (seen != null) return seen;
  const ts = wallClockMs(l.timestamp, Date.now());
  receivedAt.set(l, ts);
  return ts;
}

function isConsole(line: ChatLine): boolean {
  return line.channel === CONSOLE_CHANNEL || isSystem(line.sender);
}

type ConsoleStatus = "ok" | "failed";
type ConsoleEntry =
  | { kind: "cmd"; id: string; text: string; count: number; status?: ConsoleStatus; out: string[] }
  | { kind: "out"; id: string; text: string }
  | { kind: "status"; id: string; status: ConsoleStatus };
type Row =
  | { kind: "day"; id: string; ts: number; last: number }
  | { kind: "msg"; line: ChatLine }
  | { kind: "console"; id: string; ts: number; entries: ConsoleEntry[] };

const STATUS_LINE = /^\[(ok|failed)\]$/i;

function appendConsole(entries: ConsoleEntry[], line: ChatLine): void {
  const text = line.message.replace(/\s+$/, "");
  if (!text.trim()) return;
  const last = entries[entries.length - 1];
  if (!isSystem(line.sender)) {
    if (last?.kind === "cmd" && last.text === text) last.count += 1;
    else entries.push({ kind: "cmd", id: line.id, text, count: 1, out: [] });
    return;
  }
  const status = STATUS_LINE.exec(text.trim())?.[1]?.toLowerCase() as ConsoleStatus | undefined;
  if (last?.kind === "cmd") {
    if (status) last.status = status;
    else if (last.out[last.out.length - 1] !== text) last.out.push(text);
  } else if (status) entries.push({ kind: "status", id: line.id, status });
  else if (!(last?.kind === "out" && last.text === text)) entries.push({ kind: "out", id: line.id, text });
}

function ConsoleStatusMark({ status }: { status: ConsoleStatus }) {
  return status === "ok" ? (
    <span className={`${styles.consoleStatus} ${styles.consoleOk}`} role="img" aria-label="Done" title="Done">
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m3.5 8.5 3 3 6-7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" /></svg>
    </span>
  ) : (
    <span className={`${styles.consoleStatus} ${styles.consoleFailed}`}>failed</span>
  );
}

function ConsoleBlock({ entries, stale }: { entries: ConsoleEntry[]; stale: boolean }) {
  return (
    <div className={styles.console} role="group" aria-label="Console" data-chat-stale={stale || undefined}>
      {entries.map((e) =>
        e.kind === "cmd" ? (
          <Fragment key={e.id}>
            <div className={styles.consoleCmd} data-console="echo">
              <span className={styles.consolePrompt} aria-hidden="true">&#x203A;</span>
              <span className={styles.consoleText}>{e.text}</span>
              {e.count > 1 && <span className={styles.consoleCount} title={`Sent ${e.count} times`}>&#xD7;{e.count}</span>}
              {e.status && <ConsoleStatusMark status={e.status} />}
            </div>
            {e.out.map((o, i) => (
              <div key={i} className={styles.consoleOut} data-console="output"><span className={styles.consoleText}>{o}</span></div>
            ))}
          </Fragment>
        ) : e.kind === "out" ? (
          <div key={e.id} data-console="output"><span className={styles.consoleText}>{e.text}</span></div>
        ) : (
          <div key={e.id} data-console="status"><ConsoleStatusMark status={e.status} /></div>
        ),
      )}
    </div>
  );
}

export function ChatBubble({
  line,
  name,
  members = [],
  me,
  onOpenProfile,
  onViewProfile,
  onLocation,
  onVisitWorld,
  preview = false,
  stale = false,
}: {
  line: ChatLine;
  name: string;
  members?: NearbyPlayer[];
  me?: { address?: string; name?: string } | null;
  onOpenProfile?: (user: ProfileCardUser, e: ReactMouseEvent) => void;
  onViewProfile?: (user: ProfileCardUser, opener: HTMLElement) => void;
  onLocation?: (x: number, y: number) => void;
  onVisitWorld?: (name: string) => void;
  preview?: boolean;
  stale?: boolean;
}) {
  const color = senderColor(line.sender);
  const { base, tag } = splitName(name);
  const senderMember = findMember(members, line.sender);
  const sender: ProfileCardUser = { address: line.sender, name, picture: senderMember?.picture };
  const highlight = mentionsMe(line.message, me ?? null, buildNameIndex(members));
  const own = !!me?.address && line.sender.toLowerCase() === me.address.toLowerCase();
  const avatar = <SenderAvatar address={line.sender} src={senderMember?.picture} name={name} initials={initialOf(base)} size={28} />;
  const label = <>{base}{tag && <span className={styles.tag}>{tag}</span>}</>;

  const openSender = (e: ReactMouseEvent): void => {
    if (e.type === "contextmenu") e.preventDefault();
    onOpenProfile?.(sender, e);
  };
  const viewSender = (e: ReactMouseEvent<HTMLButtonElement>): void => {
    if (onViewProfile) onViewProfile(sender, e.currentTarget);
    else openSender(e);
  };
  const onMention = (address: string, mname: string, e: ReactMouseEvent): void => {
    if (e.type === "contextmenu") e.preventDefault();
    const m = findMember(members, address);
    onOpenProfile?.({ address, name: m?.name || `@${mname}`, picture: m?.picture }, e);
  };

  return (
    <div className={`${styles.entry} ${highlight ? styles.mentionMe : ""}`.trim()} data-chat-author={own ? "self" : "other"} data-chat-stale={stale || undefined}>
      {preview ? <span className={styles.avatarBtn} style={{ color }}>{avatar}</span> :
        <button type="button" className={styles.avatarBtn} style={{ color }} aria-label={`View ${base}`} onClick={viewSender} onContextMenu={openSender}>{avatar}</button>}
      <div className={styles.bubble}>
        {preview ? <span className={styles.name} style={{ color }}>{label}</span> :
          <button type="button" className={styles.name} style={{ color }} onClick={openSender} onContextMenu={openSender}>{label}</button>}
        <span className={styles.text}>
          <MessageText text={line.message} members={members} styles={MSG_STYLES} interactive={!preview} onMention={onMention} onLocation={(x, y) => onLocation?.(x, y)} onWorld={onVisitWorld} />
        </span>
        <time className={styles.time} dateTime={new Date(line.ts).toISOString()}>{formatTime(line.ts)}</time>
      </div>
    </div>
  );
}

function MemberRow({ member, self }: { member: NearbyPlayer; self?: string | null }) {
  const { base, tag } = splitName(memberLabel(member));
  const color = senderColor(member.address);
  const [status, setStatus] = useState<"idle" | "sending" | "sent" | "error">("idle");
  const [error, setError] = useState("");
  const own = !!self && self.toLowerCase() === member.address.toLowerCase();
  const add = () => {
    if (status === "sending") return;
    setStatus("sending");
    setError("");
    sendFriendRequest(member.address)
      .then(() => setStatus("sent"))
      .catch((e) => {
        setError(e instanceof Error ? e.message : "Could not send. Try again.");
        setStatus("error");
      });
  };
  return (
    <div className={styles.memberRow}>
      <SenderAvatar address={member.address} src={member.picture} name={base} hue={hexToHue(color)} size={40} status="online" />
      <div className={styles.memberInfo}>
        <span className={styles.memberName} style={{ color }}>
          {base}
          {tag && <span className={styles.tag}>{tag}</span>}
        </span>
        <span className={styles.memberStatus}>Online</span>
      </div>
      {!own && (
        <button
          type="button"
          className={styles.memberAdd}
          data-status={status === "sent" ? "sent" : undefined}
          aria-label={status === "sent" ? `Friend request sent to ${base}` : `Add ${base} as friend`}
          title={status === "error" ? error : "Add friend"}
          disabled={status === "sending"}
          onClick={add}
        >
          {status === "sending" ? "Sending…" : status === "sent" ? "Requested" : status === "error" ? "Failed" : "Add"}
        </button>
      )}
    </div>
  );
}

function MembersOverlay({
  members,
  self,
  onBack,
  onClose,
  closeControl = true,
  membersTitle = "Nearby",
  membersEmpty = "No one nearby",
}: {
  members: NearbyPlayer[];
  self?: string | null;
  onBack: () => void;
  onClose: () => void;
  closeControl?: boolean;
  membersTitle?: string;
  membersEmpty?: string;
}) {
  return (
    <div className={styles.membersPanel}>
      <header className={styles.membersHeader}>
        <CtrlButton variant="ghost" className={styles.glyphLg} aria-label="Back" onClick={onBack}>
          &#x2039;
        </CtrlButton>
        <span className={styles.membersTitle}>{membersTitle}</span>
        <span className={styles.membersCount}>
          <span className={styles.personIcon} aria-hidden="true">
            &#x25CF;
          </span>
          {members.length} Online
        </span>
        {closeControl && <CtrlButton variant="solid" className={styles.glyph} aria-label="Close chat" onClick={onClose}>
          &#xD7;
        </CtrlButton>}
      </header>
      <div className={styles.membersList}>
        {members.length === 0 ? (
          <div className={styles.empty}>{membersEmpty}</div>
        ) : (
          members.map((m) => <MemberRow key={m.address} member={m} self={self} />)
        )}
      </div>
    </div>
  );
}

export function ChatView({
  open,
  onToggle,
  hidden = false,
  io,
  docked = false,
  header = true,
  closeControl = true,
  title = "Nearby",
  membersTitle,
  membersEmpty,
  emptyLine,
  profileCard,
  onViewProfile,
  commands = true,
  draftValue,
  onDraftChange,
}: {
  open: boolean;
  onToggle: () => void;
  hidden?: boolean;
  io: ChatIo;
  docked?: boolean;
  header?: boolean;
  closeControl?: boolean;
  title?: string;
  membersTitle?: string;
  membersEmpty?: string;
  emptyLine?: string;
  profileCard?: (props: ProfileCardProps) => ReactNode;
  onViewProfile?: (user: ProfileCardUser, opener: HTMLElement | null) => void;
  commands?: boolean;
  draftValue?: string;
  onDraftChange?: (value: string) => void;
}) {
  const [localDraft, setLocalDraft] = useState("");
  const draft = draftValue ?? localDraft;
  const setDraft = (value: SetStateAction<string>) => {
    const next = typeof value === "function" ? value(draft) : value;
    if (onDraftChange) onDraftChange(next);
    else setLocalDraft(next);
  };
  const [sending, setSending] = useState(false);
  const [sendError, setSendError] = useState("");
  const followTail = useRef(true);
  const scrollPosition = useRef(0);
  const [newMessages, setNewMessages] = useState(false);
  const [picker, setPicker] = useState(false);
  const [showMembers, setShowMembers] = useState(false);
  const [emojiReady, setEmojiReady] = useState(() => getEmojiData() != null);
  const [scQuery, setScQuery] = useState<string | null>(null);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const [mentionQuery, setMentionQuery] = useState<string | null>(null);
  const [mentionSug, setMentionSug] = useState<NearbyPlayer[]>([]);
  const [profileTarget, setProfileTarget] = useState<{ user: ProfileCardUser; x: number; y: number } | null>(null);
  const [clearedAfter, setClearedAfter] = useState<{ line: BridgeChatLine | null } | null>(null);
  const [localLines, setLocalLines] = useState<LocalLine[]>([]);
  const [hiddenKeys, setHiddenKeys] = useState<ReadonlySet<string>>(() => new Set());
  const listRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const { chat: chatPushes, players, me, blocked, live } = io;
  const ProfileCardImpl = profileCard;
  const members = players;

  useEffect(() => {
    if (emojiReady || (scQuery == null && !picker)) return undefined;
    let alive = true;
    void loadEmojiData().then(() => {
      if (alive) setEmojiReady(true);
    });
    return () => {
      alive = false;
    };
  }, [emojiReady, scQuery, picker]);
  const suggestions = useMemo<Emoji[]>(
    () => (emojiReady && scQuery ? searchByShortcode(scQuery) : []),
    [emojiReady, scQuery],
  );

  const active = open && (docked || hovered || focused || picker);
  const bare = !active;

  const nameByAddr = useMemo(() => {
    const m = new Map<string, string>();
    for (const mem of members) if (mem.name.trim()) m.set(mem.address.toLowerCase(), mem.name);
    return m;
  }, [members]);
  const resolveName = (sender: string, pushedName?: string): string =>
    nameByAddr.get(sender.toLowerCase()) ?? (pushedName?.trim() || displaySender(sender));

  const blockedSet = useMemo(
    () => new Set(blocked.map((a) => a.toLowerCase())),
    [blocked],
  );
  const [consoleSince, setConsoleSince] = useState<number | null>(null);
  const visiblePushes = useMemo(() => {
    if (!clearedAfter) return chatPushes;
    const i = clearedAfter.line ? chatPushes.indexOf(clearedAfter.line) : -1;
    return chatPushes.slice(i + 1);
  }, [chatPushes, clearedAfter]);

  const lines = useMemo<ChatLine[]>(() => {
    const out: ChatLine[] = [];
    const visible = new Set(visiblePushes);
    const lastByKey = new Map<string, number>();
    visiblePushes.forEach((l, i) => lastByKey.set(consoleLineKey(l), i));
    const pending = new Map<BridgeChatLine, LocalLine[]>();
    const pendingByKey = new Map<string, LocalLine[]>();
    const toLine = (ll: LocalLine): ChatLine => ({ id: ll.id, ts: ll.ts, sender: "system", message: ll.message, channel: CONSOLE_CHANNEL });
    for (const ll of localLines) {
      if (ll.anchorKey != null && lastByKey.has(ll.anchorKey)) pendingByKey.set(ll.anchorKey, [...(pendingByKey.get(ll.anchorKey) ?? []), ll]);
      else if (ll.anchorKey == null && ll.anchor && visible.has(ll.anchor)) pending.set(ll.anchor, [...(pending.get(ll.anchor) ?? []), ll]);
      else out.push(toLine(ll));
    }
    visiblePushes.forEach((l, i) => {
      const key = consoleLineKey(l);
      const blockedLine = l.senderAddress && blockedSet.has(l.senderAddress.toLowerCase());
      const unsolicitedSystem = !l.senderAddress && isSystem(l.senderName ?? "") && (consoleSince === null || stampOf(l) < consoleSince);
      const commandEcho = l.channel === CONSOLE_CHANNEL && /^\/(?:goto|teleport|changerealm)(?:\s|$)/i.test(l.message?.trimStart() ?? "");
      const hiddenLine = commandEcho || unsolicitedSystem || (!l.senderAddress && !l.senderName && hiddenKeys.has(key));
      if (!blockedLine && !hiddenLine) {
        const sender = l.senderAddress || l.senderName || "system";
        const ts = stampOf(l);
        out.push({
          id: `t${ts}-${i}`,
          ts,
          sender,
          senderName: l.senderName ?? undefined,
          message: l.message ?? "",
          channel: l.channel ?? undefined,
        });
      }
      for (const ll of pending.get(l) ?? []) out.push(toLine(ll));
      if (lastByKey.get(key) === i) for (const ll of pendingByKey.get(key) ?? []) out.push(toLine(ll));
    });
    return out;
  }, [visiblePushes, localLines, blockedSet, hiddenKeys, consoleSince]);

  const rows = useMemo(() => {
    const out: Row[] = [];
    let day: Extract<Row, { kind: "day" }> = { kind: "day", id: "", ts: 0, last: 0 };
    let block: Extract<Row, { kind: "console" }> | null = null;
    for (const line of lines) {
      const id = `day-${dayKey(line.ts)}`;
      if (day.id !== id) {
        day = { kind: "day", id, ts: line.ts, last: line.ts };
        out.push(day);
        block = null;
      }
      day.last = line.ts;
      if (!isConsole(line)) {
        block = null;
        out.push({ kind: "msg", line });
        continue;
      }
      if (!block) {
        block = { kind: "console", id: `console-${line.id}`, ts: line.ts, entries: [] };
        out.push(block);
      }
      block.ts = line.ts;
      appendConsole(block.entries, line);
    }
    return out;
  }, [lines]);
  const stamps = useMemo(() => lines.map((l) => l.ts), [lines]);
  const now = useNow(stamps);
  const isStale = (ts: number): boolean => now - ts >= FRESH_MS;

  useEffect(() => {
    if (!open || hidden) return;
    const el = listRef.current;
    if (!el) return;
    if (followTail.current) el.scrollTop = el.scrollHeight;
    else {
      el.scrollTop = scrollPosition.current;
      setNewMessages(true);
    }
  }, [lines, open, hidden]);

  useEffect(() => {
    if (open && !hidden && !sending) inputRef.current?.focus({ preventScroll: true });
    else setHovered(false);
  }, [open, hidden, sending]);

  useEffect(() => {
    if (!open || hidden) return undefined;
    const onKey = (e: KeyboardEvent): void => {
      if (e.key !== "Enter" || e.altKey || e.ctrlKey || e.metaKey) return;
      const ae = document.activeElement;
      if (
        ae &&
        (ae.tagName === "INPUT" ||
          ae.tagName === "TEXTAREA" ||
          (ae instanceof HTMLElement && ae.isContentEditable))
      )
        return;
      e.preventDefault();
      inputRef.current?.focus();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, hidden]);

  useEffect(() => {
    if (!active) setShowMembers(false);
  }, [active]);

  const openIfClosed = (): void => {
    if (!open) onToggle();
  };

  const openProfile = (user: ProfileCardUser, e: ReactMouseEvent): void => {
    setProfileTarget({ user, x: e.clientX, y: e.clientY });
  };
  const insertMention = (name: string): void => {
    setDraft((d) => `${d.replace(/\s*$/, "")} @${name} `.trimStart());
    openIfClosed();
    inputRef.current?.focus();
  };

  const MENTION_RE = /@([\w-]*)$/;
  const updateDraft = (value: string): void => {
    setDraft(value);
    const m = value.match(SHORTCODE_RE);
    const code = m?.[1];
    setScQuery(code ?? null);
    const mm = value.match(MENTION_RE);
    const mention = mm?.[1];
    setMentionQuery(mention ?? null);
    const q = (mention ?? "").toLowerCase();
    setMentionSug(mm ? members.filter((p) => (p.name || p.address).toLowerCase().includes(q)).slice(0, 6) : []);
  };

  const applyMention = (member: NearbyPlayer): void => {
    const label = member.name.trim() ? member.name.split("#")[0] : member.address;
    setDraft((d) => d.replace(MENTION_RE, `@${label} `));
    setMentionSug([]);
    setMentionQuery(null);
    inputRef.current?.focus();
  };

  const applyEmoji = (glyph: string): void => {
    setDraft((d) => {
      const m = d.match(SHORTCODE_RE);
      return (m ? d.slice(0, m.index) : d) + glyph;
    });
    setScQuery(null);
    inputRef.current?.focus();
  };

  const printLocal = (message: string, anchor: BridgeChatLine | null, anchorKey?: string): void => {
    localSeq += 1;
    setLocalLines((ls) => [...ls, { id: `l${localSeq}`, ts: Date.now(), message, anchor, anchorKey }]);
  };

  const askEngineHelp = (source: ConsoleSource, anchor: BridgeChatLine | null): void => {
    void requestEngineHelp(source, io.send).then((help) => {
      if (!help || help.names.length === 0) {
        printLocal(helpText(), anchor);
        return;
      }
      setHiddenKeys((h) => new Set([...h, ...help.keys]));
      printLocal(helpText(help.names), anchor, help.keys[help.keys.length - 1]);
    });
  };

  const send = async (): Promise<void> => {
    const message = draft.trim();
    if (!message || sending) return;
    setSendError("");
    const action = commands ? dispatchCommand(message) : { kind: "send" as const, message };
    if (message.startsWith("/")) setConsoleSince(Date.now());
    const anchor = chatPushes[chatPushes.length - 1] ?? null;
    if (action.kind === "clear") {
      setClearedAfter({ line: anchor });
      setLocalLines([]);
    } else if (action.kind === "print") {
      printLocal(action.text, anchor);
    } else if (action.kind === "help") {
      if (io.console) askEngineHelp(io.console, anchor);
      else printLocal(helpText(), anchor);
    } else {
      setSending(true);
      try {
        await io.send(action.message);
      } catch (error) {
        setSendError(error instanceof Error ? error.message : "Message not sent. Try again.");
        return;
      } finally {
        setSending(false);
      }
    }
    followTail.current = true;
    setNewMessages(false);
    if (listRef.current) listRef.current.scrollTop = listRef.current.scrollHeight;
    setDraft("");
    setScQuery(null);
    setMentionSug([]);
    setMentionQuery(null);
  };

  const onKeyDown = (e: ReactKeyboardEvent): void => {
    e.stopPropagation();
    if (e.nativeEvent.isComposing) return;
    if (e.key === "Enter") {
      e.preventDefault();
      const firstMention = mentionSug[0];
      const firstEmoji = suggestions[0];
      if (firstMention) applyMention(firstMention);
      else if (firstEmoji) applyEmoji(firstEmoji.emoji);
      else void send();
    } else if (e.key === "Escape") {
      if (mentionQuery != null) {
        setMentionQuery(null);
        setMentionSug([]);
      } else if (scQuery != null) {
        setScQuery(null);
      } else if (picker) setPicker(false);
    }
  };

  const toggleEmoji = (): void => {
    openIfClosed();
    setPicker((p) => !p);
  };

  const onLocation = (x: number, y: number): void => {
    io.teleport?.(x * PARCEL_SIZE + PARCEL_SIZE / 2, y * PARCEL_SIZE + PARCEL_SIZE / 2);
  };
  const onVisitWorld = (name: string): void => {
    io.changeRealm?.(name);
  };

  if (hidden || !open) return null;

  return (
    <div
      className={`${styles.root} ${docked ? styles.docked : ""} ${open ? styles.open : ""} ${active ? styles.active : ""}`.trim()}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {header && open && active && (
        <header className={styles.nav} data-chat-chrome>
          <div className={styles.navLeft}>
            <img src={logo} alt="" className={styles.channelIcon} />
            <span className={styles.navTitle}>{title}</span>
          </div>
          <div className={styles.navRight}>
            {members.length > 0 && (
              <CtrlButton
                variant="ghost"
                shape="pill"
                active={showMembers}
                aria-label={`${members.length} nearby`}
                onClick={() => setShowMembers((s) => !s)}
              >
                <PersonIcon />
                {members.length}
              </CtrlButton>
            )}
            {closeControl && <CtrlButton variant="solid" className={styles.glyph} aria-label="Close chat" onClick={onToggle}>
              &#xD7;
            </CtrlButton>}
          </div>
        </header>
      )}

      {open && (
        <div ref={listRef} className={styles.messages} role="log" aria-label={`${title} messages`} aria-live="polite" onScroll={() => {
          const el = listRef.current;
          if (!el) return;
          scrollPosition.current = el.scrollTop;
          followTail.current = el.scrollHeight - el.clientHeight - el.scrollTop < 40;
          if (followTail.current) setNewMessages(false);
        }}>
          {rows.length === 0 ? (
            <div className={styles.empty} data-chat-chrome>
              {live
                ? emptyLine ?? `Say hello to ${title}!`
                : `Connecting to ${title} chat\u{2026}`}
            </div>
          ) : (
            rows.map((r) =>
              r.kind === "day" ? (
                <DaySeparator key={r.id} ts={r.ts} stale={isStale(r.last)} />
              ) : r.kind === "console" ? (
                <ConsoleBlock key={r.id} entries={r.entries} stale={isStale(r.ts)} />
              ) : (
                <ChatBubble
                  key={r.line.id}
                  stale={isStale(r.line.ts)}
                  line={r.line}
                  name={resolveName(r.line.sender, r.line.senderName)}
                  members={members}
                  me={me}
                  onOpenProfile={openProfile}
                  onViewProfile={onViewProfile}
                  onLocation={onLocation}
                  onVisitWorld={onVisitWorld}
                />
              ),
            )
          )}
        </div>
      )}

      {newMessages && <button type="button" className={styles.latest} onClick={() => {
        followTail.current = true;
        setNewMessages(false);
        if (listRef.current) listRef.current.scrollTop = listRef.current.scrollHeight;
      }}>New messages &#xb7; Jump to latest</button>}

      {active && picker && (
        <div className={styles.pickerWrap}>
          <EmojiPicker onPick={applyEmoji} onClose={() => setPicker(false)} />
        </div>
      )}

      {active && mentionQuery != null && mentionSug.length > 0 && (
        <div className={styles.suggest}>
          <ul className={styles.suggestList}>
            {mentionSug.map((m, i) => (
              <li key={m.address}>
                <button
                  type="button"
                  className={`${styles.suggestItem} ${i === 0 ? styles.suggestActive : ""}`.trim()}
                  onClick={() => applyMention(m)}
                >
                  <Avatar src={m.picture} name={m.name || m.address} hue={hexToHue(senderColor(m.address))} size={20} />
                  <span className={styles.suggestName}>{m.name || truncateIfAddress(m.address)}</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}

      {active && scQuery != null && emojiReady && (
        <div className={styles.suggest}>
          {suggestions.length === 0 ? (
            <div className={styles.noResults}>No results</div>
          ) : (
            <ul className={styles.suggestList}>
              {suggestions.map((e, i) => (
                <li key={e.code}>
                  <button
                    type="button"
                    className={`${styles.suggestItem} ${i === 0 ? styles.suggestActive : ""}`.trim()}
                    onClick={() => applyEmoji(e.emoji)}
                  >
                    <span className={styles.suggestGlyph}>{e.emoji}</span>
                    <span className={styles.suggestName}>{e.expression}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      <form
        className={styles.inputRow}
        data-chat-composer
        onSubmit={(e) => {
          e.preventDefault();
          void send();
        }}
      >
        <div className={styles.composerField}>
        <input
          ref={inputRef}
          className={`${styles.input} ${bare ? styles.inputBare : draft.length >= MAX_LEN * .8 ? styles.inputCounting : ""}`.trim()}
          value={draft}
          onChange={(e) => updateDraft(e.target.value)}
          onFocus={() => {
            setFocused(true);
            openIfClosed();
          }}
          onBlur={() => setFocused(false)}
          placeholder={title === "Nearby" ? "Say something to people around" : `Message ${title}`}
          disabled={sending}
          maxLength={MAX_LEN}
          onKeyDown={onKeyDown}
          aria-label={`Send a message to ${title} chat`}
        />
        {!bare && draft.length >= MAX_LEN * .8 && <CharRing len={draft.length} />}
        {!bare && (
          <CtrlButton variant="ghost" size="sm" active={picker} className={styles.emojiBtn} aria-label="Emoji" onClick={toggleEmoji} disabled={sending} data-chat-chrome>
            <Smiley />
          </CtrlButton>
        )}
        </div>
        <button type="submit" className={styles.send} hidden={!draft.trim() && !sending} disabled={sending || !draft.trim()} aria-busy={sending} aria-label={`Send message to ${title}`} title={sending ? "Sending\u2026" : "Send message"}>
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M12 19V5m-6 6 6-6 6 6" /></svg>
        </button>
      </form>
      {sendError && <p className={styles.sendError} role="alert">{sendError}</p>}

      {open && active && showMembers && (
        <MembersOverlay
          members={members}
          self={me?.address ?? null}
          closeControl={closeControl}
          membersTitle={membersTitle}
          membersEmpty={membersEmpty}
          onBack={() => setShowMembers(false)}
          onClose={() => {
            setShowMembers(false);
            onToggle();
          }}
        />
      )}

      {profileTarget &&
        ProfileCardImpl &&
        createPortal(
          <ProfileCardImpl
            user={profileTarget.user}
            x={profileTarget.x}
            y={profileTarget.y}
            onMention={(name) => {
              insertMention(name);
              setProfileTarget(null);
            }}
            onViewPassport={onViewProfile && ((user) => onViewProfile(user, inputRef.current))}
            onClose={() => setProfileTarget(null)}
          />,
          document.body,
        )}
    </div>
  );
}
