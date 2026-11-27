// Passport — the full-screen profile view opened from the chat profile menu's
// "View Profile". Header (name, wallet, copy, mutual friends, then the friendship button, Chat,
// Call, Jump to Location and the options menu),
// OVERVIEW / BADGES / PHOTOS / CREATIONS tabs, the avatar down the left (the engine's live one
// when its host gives the `avatar` slot, see PassportAvatar; else the catalyst full-body snapshot,
// falling back to the face), then badges + about-me + fields + links.

import { Button, DclLogo, EmptyState, EquippedItemCard, MaskIcon, ScrollThumb, Spinner, Tooltip, type EquippedItemCardProps } from '@dcl/one-ui'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Avatar, BlockUserIcon, Check, Close, ColorPicker, ContextMenu, Copy, FriendshipIcon, Icon, Kebab, Message, Pencil, ReportFlag, Search, Tabs, showConfirm, type ContextMenuItem, type TabItem, NameSeal } from '../../design'
import giftIcon from '../../assets/gifting/gift.png'
import jumpInIcon from '../../assets/friends/jump-in.png'
import callIcon from '../../assets/voice/call.png'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { fetchCreations, marketplaceUrl, type CreationItem, type Creations } from '@explorer-core/features/shop/shopApi'
import { nativeProfileImage } from '@hud-host'
import { CategoryIcon } from '../backpack/categoryIcons'
import { catalystThumbUrl, shortWallet, splitName, userColor } from '@explorer-core/lib/identity'
import type { Badge, Emote, PassportBackground, Profile, ProfileEdit, Wearable } from '@explorer-core/engine/protocol'
import { PROFILE_FIELDS } from '@explorer-core/features/profile/profileFields'
import { ProfileEditForm } from './ProfileEditForm'
import type { Relationship } from '../chat/ProfileCardPresentation'
import { MutualFriends } from '../friends/MutualFriends'
import type { MutualFriend } from '../session/useEngineSession'
import { PassportPhotos, type PassportPhotoActions } from '../gallery/PassportPhotos'
import { inviteSubmenuItem } from '../communities/InviteToCommunity'
import styles from './ProfilePassport.module.css'

export type PassportTab = 'overview' | 'badges' | 'photos' | 'creations'
/** What the live avatar is given: the panel's background behind it, whether it waves once loaded (the
 *  first time the passport shows it), what the column shows in its place when it cannot be drawn, and
 *  where to say that it is being drawn, so the panel opens its background over the column. */
export type PassportAvatarSlot = {
  background: PassportBackground
  wave: boolean
  picture: React.ReactNode
  onLive: (live: boolean) => void
  onReady: (ready: boolean) => void
}
type Tab = PassportTab
const PASSPORT_TABS: TabItem<Tab>[] = (['overview', 'badges', 'photos', 'creations'] as const).map((t) => ({ id: t, label: t.toUpperCase() }))
// A single selected-but-disabled tab: the same bar, holding the same height, with nothing to switch to.
const EDIT_TAB: TabItem<'edit'>[] = [{ id: 'edit', label: 'EDIT PROFILE', disabled: true }]
// unity-explorer's name colour presets (UI/ColorPicker/NameColors.asset), in its order.
const NAME_COLOR_PRESETS = [
  '#a140ff', '#d740ff', '#ff40ad', '#ff4046', '#ff9840', '#ffbd40',
  '#ffe340', '#b1ff40', '#40ff45', '#40eaff', '#4092ff', '#4740ff'
] as const

const COPIED_MS = 1600

function CopyButton({ value, label, copied }: { value: string; label: string; copied: string }): React.JSX.Element {
  const [shown, setShown] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  useEffect(() => () => clearTimeout(timer.current), [])
  const copy = (): void => {
    void navigator.clipboard
      ?.writeText(value)
      .then(() => {
        setShown(true)
        clearTimeout(timer.current)
        timer.current = setTimeout(() => setShown(false), COPIED_MS)
      })
      .catch(() => {})
  }
  return (
    <span className={styles.copyWrap}>
      <button type="button" className={styles.copy} title={`Copy ${label}`} onClick={copy}>
        <Copy size={16} strokeWidth={1.7} />
      </button>
      <span role="status" className={styles.copied} data-shown={shown || undefined}>
        {shown && (
          <>
            <Check size={18} />
            {copied}
          </>
        )}
      </span>
    </span>
  )
}

// The outlined seal with a tick on the NAME buttons.
function Verified(): React.JSX.Element {
  return (
    <svg viewBox="0 0 16 16" width="16" height="16" aria-label="verified">
      <defs>
        <linearGradient id="ppv" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#ff2d55" />
          <stop offset="1" stopColor="#c640cd" />
        </linearGradient>
      </defs>
      <path d="M8 1l1.7 1.2 2.1-.2 1 1.8 1.9.9-.5 2 .9 1.9-1.6 1.4.1 2.1-2 .6-1.1 1.8-2-.7-2 .7-1.1-1.8-2-.6.1-2.1L1.6 8.6l.9-1.9-.5-2 1.9-.9 1-1.8 2.1.2z" fill="url(#ppv)" />
      <path d="M5.5 8l1.7 1.7L10.8 6" stroke="#fff" strokeWidth="1.4" fill="none" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

const badgeLabel = (badge: Badge): string => (badge.tier != null ? `${badge.name} · ${badge.tier}` : badge.name)
// unity-explorer's badge date: month and year ("Oct. 2026").
const monthYear = (ms: number): string => `${new Date(ms).toLocaleDateString('en-US', { month: 'short' })}. ${new Date(ms).getFullYear()}`

// The badge categories, in display order: the filter pills and the grid's headings.
// The engine's Badge has no category yet; a source that sends one (the mock does) gets the grouped
// grid, any other the plain one.
const BADGE_CATEGORIES = ['Explorer', 'Collector', 'Creator', 'Socializer', 'Builder']
function groupBadges(badges: Badge[]): { category: string; badges: Badge[] }[] {
  const of = (b: Badge): string | undefined => (b as Badge & { category?: string }).category
  if (!badges.some((b) => of(b) != null)) return []
  const names = [...BADGE_CATEGORIES, ...new Set(badges.map((b) => of(b) ?? 'Other').filter((c) => !BADGE_CATEGORIES.includes(c)))]
  return names.map((category) => ({ category, badges: badges.filter((b) => (of(b) ?? 'Other') === category) })).filter((g) => g.badges.length > 0)
}

// The badge picture in a circle: the 56px tile by default, `className` for the other sizes.
function BadgeIcon({ image, alt, className = '' }: { image?: string; alt: string; className?: string }): React.JSX.Element {
  return (
    <div className={`${styles.badge} ${className}`}>
      {image ? <img src={image} alt={alt} /> : <span className={styles.badgePlaceholder} />}
    </div>
  )
}

// The Overview's compact row: icon only, name on hover.
function BadgeTile({ badge }: { badge: Badge }): React.JSX.Element {
  return (
    <Tooltip appearance="hud" label={badgeLabel(badge)} side="top">
      <BadgeIcon image={badge.image} alt={badge.name} />
    </Tooltip>
  )
}

function BadgeProgress({ done, target, bare = false }: { done: number; target: number; bare?: boolean }): React.JSX.Element {
  const pct = target > 0 ? Math.min(100, Math.round((done * 100) / target)) : 0
  return (
    <div className={styles.badgeProgress}>
      <div className={styles.badgeBar} role="progressbar" aria-valuenow={done} aria-valuemin={0} aria-valuemax={target}>
        <div className={styles.badgeBarFill} style={{ width: `${pct}%` }} />
      </div>
      {!bare && (
        <span className={styles.badgeBarValue}>
          {done}/{target}
        </span>
      )}
    </div>
  )
}

// The Badges tab's grid card, after unity-explorer's: icon, name, then the unlock month — or,
// on your own passport, how far you are from the next tier (or from a multi-step badge).
function BadgeCard({ badge, isSelf, selected, onSelect }: { badge: Badge; isSelf: boolean; selected: boolean; onSelect: (id: string) => void }): React.JSX.Element {
  const foot =
    isSelf && badge.progress != null ? (
      <>
        <BadgeProgress {...badge.progress} bare />
      </>
    ) : badge.unlockedAt != null ? (
      <time dateTime={new Date(badge.unlockedAt).toISOString()}>{monthYear(badge.unlockedAt)}</time>
    ) : (
      '—'
    )
  return (
    <button
      type="button"
      className={`${styles.badgeCard} ${selected ? styles.picked : ''} ${badge.locked ? styles.badgeLocked : ''}`}
      aria-pressed={selected}
      onClick={() => onSelect(badge.id)}
    >
      <BadgeIcon image={badge.image} alt={badge.name} />
      <div className={styles.badgeName}>{badge.name}</div>
      <div className={styles.badgeFoot}>{foot}</div>
    </button>
  )
}

// The selected badge, in place of the avatar: a big picture, when it was unlocked, what it took,
// and for tiered badges the tiers in a row (picking one shows that tier) plus, on your own
// passport, the next tier's requirement and your progress toward it.
function BadgeDetail({ badge, tierId, isSelf, onSelectTier }: { badge: Badge; tierId?: string; isSelf: boolean; onSelectTier: (id: string) => void }): React.JSX.Element {
  // Other people's passports list only the tiers they have reached, as unity-explorer does.
  const tiers = isSelf ? badge.tiers : badge.tiers?.filter((t) => t.unlockedAt != null)
  const tier = tiers?.find((t) => t.id === tierId)
  const name = tier != null ? `${badge.name} ${tier.name}` : badge.name
  const image = tier?.image ?? badge.image
  const unlockedAt = tier != null ? tier.unlockedAt : badge.unlockedAt
  const locked = tier != null ? tier.unlockedAt == null : badge.locked === true
  const description = tier?.description ?? badge.description
  const nextTier = badge.tiers?.find((t) => t.unlockedAt == null)
  return (
    <div className={styles.badgeDetail}>
      <BadgeIcon image={image} alt={name} className={`${styles.badgeHero} ${locked ? styles.badgeLocked : ''}`} />
      <h2 className={styles.badgeDetailName}>{name}</h2>
      <div className={styles.badgeDetailDate}>{unlockedAt != null ? `Unlocked: ${monthYear(unlockedAt)}` : 'Locked'}</div>
      {description && <p className={styles.badgeDetailDesc}>{description}</p>}
      {tiers != null && tiers.length > 0 && (
        <div className={styles.tierRow}>
          {tiers.map((t) => (
            <button
              key={t.id}
              type="button"
              className={`${styles.tierBtn} ${t.id === tier?.id ? styles.picked : ''} ${t.unlockedAt == null ? styles.badgeLocked : ''}`}
              aria-pressed={t.id === tier?.id}
              aria-label={`${badge.name} ${t.name}`}
              onClick={() => onSelectTier(t.id)}
            >
              <BadgeIcon image={t.image} alt="" className={styles.badgeSm} />
            </button>
          ))}
        </div>
      )}
      {isSelf && badge.progress != null && (
        <div className={styles.badgeNext}>
          {nextTier != null && (
            <>
              <div className={styles.badgeLabel}>NEXT TIER: {nextTier.name}</div>
              <div className={styles.badgeNextDesc}>{nextTier.description}</div>
            </>
          )}
          <BadgeProgress {...badge.progress} />
        </div>
      )}
    </div>
  )
}

/** The tier a badge opens on: the latest reached, else (nothing reached yet) the first. */
function defaultTier(badge: Badge): string | undefined {
  const reached = badge.tiers?.filter((t) => t.unlockedAt != null)
  return (reached != null && reached.length > 0 ? reached[reached.length - 1] : badge.tiers?.[0])?.id
}

// Read-only equipped-item tiles, 6 per row like unity-explorer's passport. No equip affordance
// (this is someone else's passport, or a view-only summary of your own) — instead the SHOP button
// deep-links to the item's shop page. The link is resolved by the bridge (it needs the item's
// on-chain contract + item id); items with no listing — base wearables and emotes — simply show no
// button.
type EquippedTile = EquippedItemCardProps & { key: string }

const wearableTile = (w: Wearable): EquippedTile => ({
  key: w.urn,
  thumbnail: w.thumbnail ?? catalystThumbUrl(w.urn),
  name: w.name,
  rarity: w.rarity,
  shopUrl: w.shopUrl,
  categoryIcon: <CategoryIcon category={w.category} size={15} />
})

const emoteTile = (e: Emote, onPlay?: (urn: string) => void): EquippedTile => {
  const tile = {
    // Keyed by slot too: the same emote can sit in more than one wheel slot (the equipped set is
    // deduped, the wheel isn't), and a duplicate key drops the second tile.
    key: `${e.urn}:${e.slot}`,
    thumbnail: e.thumbnail ?? catalystThumbUrl(e.urn),
    name: e.name,
    rarity: e.rarity,
    shopUrl: e.shopUrl,
    categoryIcon: <Icon name="emotes" size={15} />
  }
  return onPlay != null ? { ...tile, onAction: () => onPlay(e.urn), actionLabel: `Play ${e.name}` } : tile
}

function EquippedRow({ tiles }: { tiles: EquippedTile[] }): React.JSX.Element {
  return (
    <div className={styles.equippedRow}>
      {tiles.map(({ key, ...tile }) => <EquippedItemCard key={key} {...tile} />)}
    </div>
  )
}

// The Creations tab's tiles, after unity-explorer's CreationsDetailsPassportModuleController: an item
// on primary sale offers Buy with its Credits price, any other item View. Both open the item's shop
// page, as unity-explorer does whenever it can't buy in-world. The button stays on the card, as in
// unity-explorer, so a hovered card doesn't cover the row below.
const creationTile = (item: CreationItem, emote: boolean): EquippedTile => ({
  key: item.urn || item.id,
  thumbnail: item.thumbnail,
  name: item.name,
  rarity: item.rarity,
  shopUrl: item.url ? marketplaceUrl(item) : undefined,
  shopLabel: item.isOnSale ? 'Buy' : 'View',
  pinShop: true,
  price: item.isOnSale && (item.priceCredits ?? 0) > 0 ? `${item.priceCredits} Credits` : undefined,
  categoryIcon: emote ? <Icon name="emotes" size={15} /> : <CategoryIcon category={item.data?.wearable?.category ?? item.category} size={15} />
})

type CreationsLoad = { status: 'loading' } | { status: 'ready'; creations: Creations } | { status: 'error' }

/** The Creations tab. Mounted only while the tab is open, so each visit loads afresh, like
 *  unity-explorer's passport. */
function CreationsSection({ address }: { address: string }): React.JSX.Element {
  const [load, setLoad] = useState<CreationsLoad>({ status: 'loading' })
  const [attempt, setAttempt] = useState(0)
  useEffect(() => {
    const ctl = new AbortController()
    setLoad({ status: 'loading' })
    // A reply that lands after the address changed (or the tab closed) belongs to the old request.
    fetchCreations(address, ctl.signal).then(
      (creations) => {
        if (!ctl.signal.aborted) setLoad({ status: 'ready', creations })
      },
      () => {
        if (!ctl.signal.aborted) setLoad({ status: 'error' })
      }
    )
    return () => ctl.abort()
  }, [address, attempt])

  if (load.status === 'loading') {
    return (
      <section className={`${styles.card} ${styles.cardFill}`}>
        <div className={styles.emptyState}>
          <Spinner variant="thin" size={34} />
        </div>
      </section>
    )
  }
  if (load.status === 'error') {
    return (
      <section className={styles.card}>
        <EmptyState
          variant="inline"
          tone="error"
          title="Couldn't load the creations"
          subtitle="Please try again."
          actions={[{ label: 'Retry', onClick: () => setAttempt((n) => n + 1) }]}
        />
      </section>
    )
  }
  const { wearables, emotes } = load.creations
  if (wearables.length === 0 && emotes.length === 0) {
    return (
      <section className={`${styles.card} ${styles.cardFill}`}>
        <div className={styles.emptyState}>
          <span className={styles.emptyIcon}>
            <Search size={34} />
          </span>
          <p className={styles.emptyText}>There are no creations to show yet</p>
        </div>
      </section>
    )
  }
  return (
    <section className={styles.card}>
      {wearables.length > 0 && (
        <>
          <h2 className={styles.cardTitle}>Wearables</h2>
          <EquippedRow tiles={wearables.map((w) => creationTile(w, false))} />
        </>
      )}
      {emotes.length > 0 && (
        <>
          <h2 className={styles.cardTitle}>Emotes</h2>
          <EquippedRow tiles={emotes.map((e) => creationTile(e, true))} />
        </>
      )}
    </section>
  )
}

/** Everything the own-profile edit mode needs. Absent = view only, which is every OTHER user's
 *  passport and your own until the session has a profile to edit. */
export interface PassportEditing {
  saving: boolean
  error: string | null
  save: (edit: ProfileEdit) => void
  dismissError: () => void
  /** Open the name editor — its own popup, since picking a claimed NAME is a different shape of
   *  choice from the rest of the form (see NameEditModal). */
  editName: () => void
  /** Preview a claimed name's colour; the passport's owner deploys it when the passport closes. */
  pickNameColor: (hex: string) => void
}

export function ProfilePassport({
  profile,
  relationship = 'none',
  blockedBy = false,
  isSelf = false,
  editing,
  onAddFriend,
  onUnfriend,
  onMessage,
  onCall,
  onCancelRequest,
  onAcceptRequest,
  onUnblock,
  onBlock,
  onReport,
  onJump,
  onGift,
  inviteItems = [],
  mutuals,
  onClose,
  onDirtyChange,
  photoActions,
  initialTab = 'overview',
  avatar,
  avatarPending = false,
  onPlayEmote
}: {
  profile: Profile
  /** Relationship of the local user to this profile — drives the header friendship button. */
  relationship?: Relationship
  /** This user has blocked you. The passport then offers no Chat, Call or friendship button;
   *  Block and Report stay in the options menu. */
  blockedBy?: boolean
  /** Your own passport — hides the friend action (you can't friend yourself). */
  isSelf?: boolean
  /** Own-profile edit mode. Only offered when this is your passport. */
  editing?: PassportEditing
  onAddFriend?: (address: string) => void
  onUnfriend?: (address: string) => void
  /** Open a DM with this user. */
  onMessage?: (address: string) => void
  /** Ring the user (a private voice call). */
  onCall?: (address: string) => void
  onCancelRequest?: (address: string) => void
  onAcceptRequest?: (address: string) => void
  onUnblock?: (address: string) => void
  onBlock?: (address: string) => void
  onReport?: (address: string) => void
  /** Travel to where this friend is; set only for a friend who is not offline (unity-explorer's
   *  Jump to Location, in the header and first in the options menu). */
  onJump?: (address: string) => void
  /** Open the gift picker for this user; absent where this session can't gift. */
  onGift?: (address: string) => void
  /** The Invite to Community submenu's communities; the option shows only when there are some. */
  inviteItems?: ContextMenuItem[]
  mutuals?: MutualFriend[]
  onClose: () => void
  /** Announce unsaved edits, so the popup layer can refuse to close on a stray backdrop click. */
  onDirtyChange?: (dirty: boolean) => void
  /** The Photos tab's writes and viewer (signed through the session); without them it only lists. */
  photoActions?: PassportPhotoActions
  /** The tab the passport opens on. */
  initialTab?: PassportTab
  /** The engine's live avatar for the left column, on every tab but Badges. Without it the column shows the
   *  profile's picture. */
  avatar?: (slot: PassportAvatarSlot) => React.ReactNode
  /** The avatar is not known yet: the column waits with a spinner instead of showing the picture. */
  avatarPending?: boolean
  /** Makes the equipped emotes' tiles buttons that play them. */
  onPlayEmote?: (urn: string) => void
}): React.JSX.Element {
  const [tab, setTab] = useState<Tab>(initialTab)
  // The Badges tab's selection: a badge id and, for tiered badges, a tier id. Both fall back —
  // to the first badge, and to the badge's latest reached tier — so the panel is never empty.
  const [picked, setPicked] = useState<{ badge?: string; tier?: string }>({})
  const badges = profile.badges ?? []
  const unlockedBadges = badges.filter((b) => !b.locked)
  const badgeGroups = groupBadges(badges)
  const [badgeCategory, setBadgeCategory] = useState('All')
  const shownCategory = badgeGroups.some((g) => g.category === badgeCategory) ? badgeCategory : 'All'
  const selectedBadge = badges.find((b) => b.id === picked.badge) ?? badges[0]
  const selectedTier = selectedBadge?.tiers?.some((t) => t.id === picked.tier) ? picked.tier : selectedBadge != null ? defaultTier(selectedBadge) : undefined
  const [editMode, setEditMode] = useState(false)
  const [liveAvatar, setLiveAvatar] = useState(false)
  const [avatarReady, setAvatarReady] = useState(false)
  // It waves the first time this passport shows it, not when it comes back from the Badges tab.
  const waved = useRef(false)
  const firstShow = !waved.current
  useEffect(() => {
    if (avatar != null && !avatarPending && tab !== 'badges') waved.current = true
  })
  const canEdit = isSelf && editing != null
  // Someone you blocked stays "blocked" even if they blocked you too: UNBLOCK is still yours to use.
  const theyBlockedYou = blockedBy && relationship !== 'blocked'
  const canTalk = !isSelf && relationship !== 'blocked' && !blockedBy
  const [menuOpen, setMenuOpen] = useState(false)
  const { base, tag } = splitName(profile.name)
  const claimed = profile.hasClaimedName
  const nameColor = userColor(profile.address, profile.name, claimed, profile.nameColor)
  const options: ContextMenuItem[] = []
  const pick = (fn: (address: string) => void) => (): void => {
    setMenuOpen(false)
    fn(profile.address)
  }
  if (!isSelf && onJump != null) {
    options.push({ label: 'Jump to Location', icon: <img src={jumpInIcon} alt="" width={16} height={16} />, onClick: pick(onJump) })
  }
  if (!isSelf && onGift != null) options.push({ label: 'Gift', icon: <MaskIcon src={giftIcon} size={16} />, onClick: pick(onGift) })
  const inviteEntry = isSelf ? null : inviteSubmenuItem(inviteItems, () => setMenuOpen(false))
  if (inviteEntry != null) options.push(inviteEntry)
  // Block and Report together at the end, past a separator, as unity-explorer's passport menu
  const destructive: ContextMenuItem[] = []
  if (!isSelf && relationship !== 'blocked' && onBlock != null) {
    destructive.push({ label: 'Block', icon: <BlockUserIcon />, danger: true, onClick: pick(onBlock) })
  }
  if (!isSelf && onReport != null) destructive.push({ label: 'Report', icon: <ReportFlag />, danger: true, onClick: pick(onReport) })
  if (options.length > 0 && destructive.length > 0) options.push({ kind: 'separator' })
  options.push(...destructive)
  if (options.length > 0) {
    options.unshift({
      kind: 'header',
      name: base,
      tag: claimed ? undefined : tag || undefined,
      address: profile.address,
      avatar: <Avatar src={profile.picture} name={base} color={nameColor} size={40} />
    })
  }
  const menuRef = useRef<HTMLDivElement>(null)
  const scroller = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!menuOpen) return
    const onDown = (e: MouseEvent): void => {
      if (menuRef.current != null && !menuRef.current.contains(e.target as Node)) setMenuOpen(false)
    }
    document.addEventListener('mousedown', onDown)
    const off = registerCancelLayer(() => setMenuOpen(false))
    return () => {
      document.removeEventListener('mousedown', onDown)
      off()
    }
  }, [menuOpen])
  // SAVE sits in the header rather than at the end of the form: the form is taller than the panel,
  // so a footer button is below the fold and easy to miss entirely.
  const saveRef = useRef<(() => void) | null>(null)
  const [editStatus, setEditStatus] = useState({ dirty: false, canSave: false })
  const onStatusChange = useCallback(
    (s: { dirty: boolean; canSave: boolean }) =>
      setEditStatus((prev) => (prev.dirty === s.dirty && prev.canSave === s.canSave ? prev : s)),
    []
  )
  // Only edit mode has unsaved state; leaving it (save, cancel) clears the guard.
  const unsaved = editMode && editStatus.dirty

  // Leaving edit mode without saving. Asks the same question the popup layer asks when you close
  // the whole passport with work in progress — this is the same loss, by a shorter route.
  const cancelEdit = async (): Promise<void> => {
    if (
      unsaved &&
      !(await showConfirm({
        title: 'Discard changes?',
        body: 'Your edits to this profile will be lost.',
        confirmLabel: 'Discard',
        cancelLabel: 'Keep editing'
      }))
    ) {
      return
    }
    editing?.dismissError()
    setEditMode(false)
  }
  const dirtyCb = useRef(onDirtyChange)
  dirtyCb.current = onDirtyChange
  useEffect(() => {
    dirtyCb.current?.(unsaved)
    return () => dirtyCb.current?.(false)
  }, [unsaved])
  // Leave edit mode only once a save has actually landed: a rejected deploy comes back as an error
  // on `editing`, and closing the form on click would throw away both the error and the user's
  // unsaved text.
  const wasSaving = useRef(false)
  useEffect(() => {
    if (editing == null) return
    if (wasSaving.current && !editing.saving && editing.error == null) setEditMode(false)
    wasSaving.current = editing.saving
  }, [editing])
  // (Escape is handled centrally by the popup stack — see popups.tsx.)
  const fields = PROFILE_FIELDS.filter(({ key }) => profile.info?.[key])
  const hasBadges = unlockedBadges.length > 0
  const hasAbout = !!profile.description || fields.length > 0 || (profile.links?.length ?? 0) > 0
  // The body shape isn't a collectible you can shop for — Unity skips it before filling the grid
  // (EquippedItems_PassportModuleController.SetGridElements). It skips hidden categories too, but
  // that needs each item's hides/replaces metadata, which the equipped set doesn't carry.
  const wearables = (profile.equippedWearables ?? []).filter((w) => w.category !== 'body_shape')
  const hasWearables = wearables.length > 0
  const hasEmotes = (profile.equippedEmotes?.length ?? 0) > 0
  const hasEquipped = hasWearables || hasEmotes
  const hasOverview = hasBadges || hasAbout || hasEquipped
  const startEdit = (): void => {
    setTab('overview')
    setEditMode(true)
  }

  const showsBadge = tab === 'badges' && !editMode && selectedBadge != null
  // the live avatar is hidden on the Badges tab, badge picked or not
  const showsAvatar = !showsBadge && !(avatar != null && tab === 'badges' && !editMode)
  const picture = profile.bodyImage ? (
    <img className={styles.avatarImg} src={nativeProfileImage(profile.bodyImage)} alt={base} />
  ) : (
    <Avatar src={profile.picture} name={base} color={nameColor} size={180} />
  )

  return (
    // The dimmed scrim + click-outside-to-close are owned by the popup layer (openPassport →
    // PopupHost); this is just the panel. stopPropagation keeps a click inside it (tabs, copy, links)
    // from reaching the scrim and closing the passport.
    <div className={styles.panel} data-tab={tab} data-passport-panel="" data-live-avatar={liveAvatar && showsAvatar ? '' : undefined} onClick={(e) => e.stopPropagation()}>
          {/* --- left: the avatar — the catalyst full-body snapshot,
                  falling back to the 2D face if the body render isn't available. --- */}
          <div className={styles.avatarCol} data-passport-avatar={showsBadge ? undefined : ''}>
            {showsBadge ? (
              <BadgeDetail badge={selectedBadge} tierId={selectedTier} isSelf={isSelf} onSelectTier={(tier) => setPicked({ badge: selectedBadge.id, tier })} />
            ) : avatarPending ? (
              <Spinner variant="thin" size={70} className={styles.avatarPending} />
            ) : avatar == null ? (
              picture
            ) : showsAvatar ? (
              avatar({ background: tab === 'photos' || tab === 'creations' ? tab : 'overview', wave: firstShow, picture, onLive: setLiveAvatar, onReady: setAvatarReady })
            ) : null}
          </div>

          <div className={styles.main}>
        {/* --- header --- */}
        <header className={styles.head}>
          <div>
            <div className={styles.nameRow}>
              <span className={styles.name} style={{ '--name-color': nameColor } as React.CSSProperties}>{base}</span>
              {!claimed && tag && <span className={styles.tag}>{tag}</span>}
              {claimed && <Verified />}
              {profile.official === true && (
                <span className={styles.official}>
                  <DclLogo size={16} />
                  DCL Foundation
                </span>
              )}
              <CopyButton value={claimed ? base : profile.name} label="name" copied="Name copied!" />
              {canEdit && (
                <button type="button" className={styles.squareBtn} aria-label="Edit name" onClick={editing.editName}>
                  <Pencil size={20} />
                </button>
              )}
              {/* CLAIM NAME shows beside an unclaimed name; it opens the name
                  editor, which is where a claimed name is picked. */}
              {canEdit && !claimed && (
                <button type="button" className={styles.claimBtn} onClick={editing.editName}>
                  <NameSeal />
                  CLAIM NAME
                </button>
              )}
              {/* A claimed name's colour, as unity-explorer offers it: own passport only, since an
                  unclaimed name always shows its derived colour. */}
              {canEdit && claimed && (
                <ColorPicker label="Name color" value={nameColor} presets={NAME_COLOR_PRESETS} hueOnly swatch onChange={editing.pickNameColor} />
              )}
            </div>
            <div className={styles.addrRow}>
              <span className={styles.addr}>{shortWallet(profile.address)}</span>
              <CopyButton value={profile.address} label="address" copied="User ID copied!" />
            </div>
            {!isSelf && <MutualFriends list={mutuals} size={20} className={styles.mutual} />}
          </div>
          <div className={styles.headActions}>
            {/* Tips open below: the panel clips at its top edge, which a tip above these buttons would cross. */}
            {!isSelf &&
              !theyBlockedYou &&
              (relationship === 'friend' ? (
                <button type="button" className={`${styles.headBtn} ${styles.friendBtn} ${styles.headBtnSwap}`} data-state="friend" onClick={() => onUnfriend?.(profile.address)}>
                  <span className={styles.idle}>
                    <FriendshipIcon kind="friend" />
                    FRIEND
                  </span>
                  <span className={styles.hover}>
                    <FriendshipIcon kind="remove" />
                    REMOVE FRIEND
                  </span>
                </button>
              ) : relationship === 'requested' ? (
                <button type="button" className={`${styles.headBtn} ${styles.friendBtn} ${styles.headBtnSwap}`} data-state="requested" onClick={() => onCancelRequest?.(profile.address)}>
                  <span className={styles.idle}>
                    <FriendshipIcon kind="pending" />
                    REQUEST SENT
                  </span>
                  <span className={styles.hover}>
                    <FriendshipIcon kind="remove" />
                    CANCEL REQUEST
                  </span>
                </button>
              ) : relationship === 'incoming' ? (
                <button type="button" className={`${styles.headBtn} ${styles.friendBtn}`} data-state="offer" onClick={() => onAcceptRequest?.(profile.address)}>
                  <FriendshipIcon kind="add" />
                  ACCEPT FRIEND
                </button>
              ) : relationship === 'blocked' ? (
                <button type="button" className={`${styles.headBtn} ${styles.friendBtn} ${styles.headBtnSwap}`} data-state="blocked" onClick={() => onUnblock?.(profile.address)}>
                  <span className={styles.idle}>
                    <FriendshipIcon kind="blocked" />
                    BLOCKED
                  </span>
                  <span className={styles.hover}>UNBLOCK</span>
                </button>
              ) : (
                <button type="button" className={`${styles.headBtn} ${styles.friendBtn}`} data-state="offer" onClick={() => onAddFriend?.(profile.address)}>
                  <FriendshipIcon kind="add" />
                  ADD FRIEND
                </button>
              ))}
            {canTalk && onMessage != null && (
              <Tooltip appearance="hud" label="Chat" side="bottom">
                <button type="button" className={`${styles.headBtn} ${styles.headIconBtn}`} aria-label="Chat" onClick={() => onMessage(profile.address)}>
                  <Message size={24} lines />
                </button>
              </Tooltip>
            )}
            {canTalk && onCall != null && (
              <Tooltip appearance="hud" label="Call" side="bottom">
                <button type="button" className={`${styles.headBtn} ${styles.headIconBtn}`} aria-label="Call" onClick={() => onCall(profile.address)}>
                  <MaskIcon src={callIcon} size={24} />
                </button>
              </Tooltip>
            )}
            {!isSelf && onJump != null && (
              <Tooltip appearance="hud" label="Jump To Location" side="bottom">
                <button type="button" className={`${styles.headBtn} ${styles.headIconBtn}`} aria-label="Jump to Location" onClick={() => onJump(profile.address)}>
                  <img src={jumpInIcon} alt="" width={24} height={24} />
                </button>
              </Tooltip>
            )}
            {options.length > 0 && (
              <div ref={menuRef} className={styles.menuWrap}>
                <Tooltip appearance="hud" label="Menu" side="bottom" disabled={menuOpen}>
                  <button type="button" className={`${styles.headBtn} ${styles.headIconBtn}`} aria-label="More options" aria-haspopup="menu" aria-expanded={menuOpen} onClick={() => setMenuOpen((o) => !o)}>
                    <Kebab size={24} r={2.2} vertical />
                  </button>
                </Tooltip>
                {menuOpen && (
                  <div className={styles.menu}>
                    <ContextMenu items={options} />
                  </div>
                )}
              </div>
            )}
            {canEdit && editMode && (
              <Button appearance="hud" variant="primary" disabled={!editStatus.canSave} onClick={() => saveRef.current?.()}>
                {editing.saving ? 'SAVING…' : 'SAVE'}
              </Button>
            )}
            {canEdit && editMode ? (
              <Button appearance="hud" variant="secondary" onClick={() => void cancelEdit()} disabled={editing.saving}>
                CANCEL
              </Button>
            ) : (
              <button type="button" className={styles.close} aria-label="Close" onClick={onClose}>
                <Close size={22} strokeWidth={3.2} />
              </button>
            )}
          </div>
        </header>

        {/* --- tabs (edit mode is overview-scoped, so they stand down while it's open) --- */}
        {!editMode && (
          <Tabs variant="underline" className={styles.tabs} items={PASSPORT_TABS} value={tab} onChange={setTab} aria-label="Passport sections" />
        )}
        {/* Edit mode has no tabs to offer, but it keeps the bar: dropping it shifts the avatar and
            everything below it up by its height, so clicking EDIT PROFILE jumped the whole panel. */}
        {editMode && (
          <Tabs variant="underline" className={styles.tabs} items={EDIT_TAB} value="edit" onChange={() => {}} aria-label="Passport sections" />
        )}

          {/* --- right: tab content --- */}
          <div className={styles.scrollWrap}>
          <div ref={scroller} className={styles.content}>
            {/* Edit mode replaces the About card (name, bio, fields and links are exactly what it
                covers) and leaves the equipped/badges sections below it — those are the Backpack's
                to change, not the passport's. */}
            {editMode && editing != null && (
              <ProfileEditForm
                profile={profile}
                saving={editing.saving}
                error={editing.error}
                onSave={editing.save}
                onStatusChange={onStatusChange}
                saveRef={saveRef}
                onDismissError={editing.dismissError}
              />
            )}
            {tab === 'overview' && !hasOverview && !editMode && !canEdit && (
              <div className={styles.empty}>This profile has no details to show yet.</div>
            )}
            {tab === 'overview' && (hasOverview || canEdit) && (
              <>
                {/* Rendered for your own passport even when empty: the pencil in its corner is how
                    profile editing is reached, so an untouched profile still has a way in. */}
                {(hasAbout || canEdit) && !editMode && (
                <section className={styles.card}>
                  <div className={styles.cardHead}>
                    <h2 className={styles.cardTitle}>About Me</h2>
                    {canEdit && (
                      <button type="button" className={`${styles.squareBtn} ${styles.cardEdit}`} aria-label="Edit profile" onClick={startEdit}>
                        <Pencil size={20} />
                      </button>
                    )}
                  </div>
                  <p className={styles.about}>{profile.description || (canEdit ? 'No intro.' : '')}</p>
                  {fields.length > 0 && (
                    <div className={styles.fields}>
                      {fields.map(({ key, label }) => (
                        <div key={key} className={styles.field}>
                          <span className={styles.fieldLabel}>{label}</span>
                          <span className={styles.fieldValue}>{profile.info?.[key]}</span>
                        </div>
                      ))}
                    </div>
                  )}
                  {((profile.links?.length ?? 0) > 0 || canEdit) && (
                    <>
                      <div className={styles.cardHead}>
                        <h2 className={styles.cardTitle}>Links</h2>
                        {canEdit && (
                          <button type="button" className={`${styles.squareBtn} ${styles.cardEdit}`} aria-label="Edit links" onClick={startEdit}>
                            <Pencil size={20} />
                          </button>
                        )}
                      </div>
                      {profile.links != null && profile.links.length > 0 ? (
                        <div className={styles.links}>
                          {profile.links.map((l) => (
                            <a key={l.url} className={styles.link} href={l.url} target="_blank" rel="noreferrer">
                              {l.title}
                            </a>
                          ))}
                        </div>
                      ) : (
                        <p className={styles.about}>No links.</p>
                      )}
                    </>
                  )}
                </section>
                )}
                {hasEquipped && (
                  <section className={styles.card}>
                    {/* One grid, wearables then emotes. */}
                    <h2 className={styles.cardTitle}>Equipped Items</h2>
                    <EquippedRow tiles={[...wearables.map(wearableTile), ...(profile.equippedEmotes ?? []).map((e) => emoteTile(e, avatarReady && showsAvatar ? onPlayEmote : undefined))]} />
                  </section>
                )}
                {hasBadges && (
                  <section className={styles.card}>
                    <h2 className={styles.cardTitle}>Badges</h2>
                    <div className={styles.badgeRow}>
                      {unlockedBadges.map((b) => <BadgeTile key={b.id} badge={b} />)}
                    </div>
                  </section>
                )}
              </>
            )}

            {tab === 'badges' && (
              <section className={styles.card}>
                {badgeGroups.length > 0 ? (
                  <>
                    <div className={styles.badgePills} role="group" aria-label="Badge categories">
                      {['All', ...BADGE_CATEGORIES].map((c) => (
                        <button key={c} type="button" aria-pressed={c === shownCategory} className={styles.badgePill} onClick={() => setBadgeCategory(c)}>
                          {c}
                        </button>
                      ))}
                    </div>
                    {badgeGroups
                      .filter((g) => shownCategory === 'All' || g.category === shownCategory)
                      .map((g) => (
                        <div key={g.category} className={styles.badgeGroup}>
                          <h3 className={styles.badgeGroupTitle}>{g.category}</h3>
                          <div className={styles.badgeGrid}>
                            {g.badges.map((b) => (
                              <BadgeCard key={b.id} badge={b} isSelf={isSelf} selected={b.id === selectedBadge?.id} onSelect={(badge) => setPicked({ badge })} />
                            ))}
                          </div>
                        </div>
                      ))}
                  </>
                ) : badges.length > 0 ? (
                  <div className={styles.badgeGrid}>
                    {badges.map((b) => (
                      <BadgeCard key={b.id} badge={b} isSelf={isSelf} selected={b.id === selectedBadge?.id} onSelect={(badge) => setPicked({ badge })} />
                    ))}
                  </div>
                ) : (
                  <div className={styles.empty}>No badges yet.</div>
                )}
              </section>
            )}

            {tab === 'photos' && (
              <PassportPhotos
                key={profile.address.toLowerCase()}
                address={profile.address}
                isSelf={isSelf}
                actions={photoActions}
                empty={
                  <section className={`${styles.card} ${styles.cardFill}`}>
                    <div className={styles.emptyState}>
                      <span className={styles.emptyIcon}>
                        <Icon name="gallery" size={34} />
                      </span>
                      <p className={styles.emptyTitle}>No photos here yet!</p>
                      {isSelf && <p className={styles.emptySub}>✨ Set your photos as public to make them visible in your Profile &amp; Communities ✨</p>}
                    </div>
                  </section>
                }
              />
            )}

            {/* Keyed on the lowercased address: the passport opens with the address it was given, then
                switches to the stored profile's lowercase one, which is the same creator. */}
            {tab === 'creations' && <CreationsSection key={profile.address.toLowerCase()} address={profile.address.toLowerCase()} />}
          </div>
          <ScrollThumb target={scroller} className={styles.thumb} />
          </div>
          </div>
      </div>
  )
}
