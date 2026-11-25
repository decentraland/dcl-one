// Design-system storybook — the living reference for tokens + primitives, matching
// the Explorer 2.0 Figma. Open with ?showcase=1. Add every new primitive here: a section in
// ShowcaseIndex.ts naming it, rendered below by that name. The UI gallery (library/ui/gallery) shows
// each section on a page of its own, and its coverage test fails when a component index.ts exports
// is not mounted by a section that names it. The primitives come from the design entry ('.'), as the
// features import them: that test watches the entry. Sections use local data only: no network,
// wallet or engine call.

import { Button, CharCounter, CodeInput, CountBadge, EmptyState, FieldLabel, Panel, Slider, Spinner, Switch } from '@dcl/one-ui'
import { useId, useRef, useState, useSyncExternalStore } from 'react'
import { getHudScale, subscribeHudScale } from '@explorer-core/lib/hudScale'
import {
  AnchoredMenu,
  ARM_MS,
  ArmedButton,
  Avatar,
  Bag,
  BrowseControl,
  BrowseLoading,
  BrowsePanel,
  BrowseToolbar,
  Chip,
  Close,
  ColorPicker,
  ContextMenu,
  ControlButton,
  DateField,
  DclLogo,
  Dropdown,
  EquippedItemCard,
  HeaderButton,
  HintsButton,
  Icon,
  IconButton,
  KebabMenu,
  KeyCap,
  LogoPattern,
  MaskIcon,
  MenuDivider,
  MenuList,
  MenuPanel,
  MenuRow,
  Modal,
  ModalActions,
  ModalShell,
  ModalTitle,
  OptionMenu,
  Pager,
  People,
  Pin,
  PopupHost,
  Rail,
  ScrollThumb,
  SearchField,
  Select,
  showConfirm,
  showDialog,
  showToast,
  Submenu,
  SubmitField,
  Tabs,
  TextArea,
  TextInput,
  ToastHost,
  VerifiedBadge,
  WearableCard,
  type IconName,
  type Rarity
} from '.'
import { SHOWCASE_INDEX, type ShowcaseName } from './ShowcaseIndex'
import reelShareIcon from '../assets/camera-reel/share-x.png'
import reelLinkIcon from '../assets/camera-reel/copy-link.png'
import reelDownloadIcon from '../assets/camera-reel/download.png'
import reelDeleteIcon from '../assets/camera-reel/delete.png'
// Local stand-ins for the pictures a profile and an emote load from servers.
import samplePicture from '../assets/lobby/default-profile-thumbnail.png'
import sampleEmote from '../assets/backpack/silhouette-1.webp'
import { ChatBubble, DaySeparator, MemberRow } from '../features/chat/Chat'
import { seedProfiles } from '../features/session/profileStore'
import { EmoteSlot } from '../features/emotes/EmoteSlot'
import streamBadge from '../assets/voice/stream-badge.png'

// A chat bubble resolves its sender through the profile store, so the demo line's author is seeded.
const MOJITO = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'
seedProfiles([{ address: MOJITO, name: 'Mojito' }])
// The chat lines' time: today at a fixed hour, so they read the same on every load.
const CHAT_TS = new Date().setHours(9, 41, 0, 0)

const EMOTE_RARITIES = ['base', 'common', 'uncommon', 'rare', 'epic', 'legendary', 'mythic', 'unique', 'exotic']

const NAV: { icon: IconName; label: string }[] = [
  { icon: 'profile', label: 'Profile' },
  { icon: 'notifications', label: 'Notifications' },
  { icon: 'map', label: 'Map' },
  { icon: 'communities', label: 'Communities' },
  { icon: 'backpack', label: 'Backpack' },
  { icon: 'settings', label: 'Settings' },
  { icon: 'mic', label: 'Voice chat' },
  { icon: 'friends', label: 'Friends' },
  { icon: 'chat', label: 'Chat' },
  { icon: 'emotes', label: 'Emotes' },
  { icon: 'help', label: 'Help' },
  { icon: 'create', label: 'Create' }
]

// DCL named palette (Explorer 2.0). token = CSS var when one exists.
const PALETTE: { name: string; hex: string; token?: string; dark?: boolean }[] = [
  { name: 'Ruby', hex: '#ff2d55', token: '--brand' },
  { name: 'Snow', hex: '#fcfcfc', token: '--text', dark: true },
  { name: 'Shadow', hex: '#161518', token: '--panel' },
  { name: 'Yellow', hex: '#ffc95b', token: '--gold', dark: true },
  { name: 'Pearl', hex: '#ecebed', token: '--pearl', dark: true },
  { name: 'Green', hex: '#30cd00', token: '--green', dark: true },
  { name: 'Lavender', hex: '#c640cd', token: '--lavender' },
  { name: 'Purple', hex: '#982de2', token: '--purple' },
  { name: 'Red', hex: '#ff0000', token: '--red' }
]

// Rarity name colors (used for player names / avatars). No tokens — domain palette.
const RARITY: { name: string; hex: string }[] = [
  { name: 'Common', hex: '#73d3d3' },
  { name: 'Common Light', hex: '#acf8f8' },
  { name: 'Uncommon', hex: '#ff8362' },
  { name: 'Epic Light', hex: '#81e1ff' },
  { name: 'Legendary', hex: '#a14bf3' },
  { name: 'Legendary Light', hex: '#e8b9ff' },
  { name: 'Mythic', hex: '#ff4bed' },
  { name: 'Unique', hex: '#fea217' },
  { name: 'Exotic', hex: '#caff73' },
  { name: 'Orange', hex: '#ff7439' },
  { name: 'Melon', hex: '#ffa25a' },
  { name: 'Periwinkle', hex: '#a0abff' }
]

const TYPE: { name: string; token: string; weight: number }[] = [
  { name: 'Display', token: '--fs-display', weight: 800 },
  { name: 'Title', token: '--fs-title', weight: 700 },
  { name: 'Large', token: '--fs-lg', weight: 600 },
  { name: 'Body (md)', token: '--fs-md', weight: 500 },
  { name: 'Small', token: '--fs-sm', weight: 400 },
  { name: 'XSmall', token: '--fs-xs', weight: 400 }
]

const RADII: { name: string; token: string }[] = [
  { name: 'control', token: '--r-control' },
  { name: 'card', token: '--r-card' },
  { name: 'panel', token: '--r-panel' },
  { name: 'pill', token: '--r-pill' }
]

const RESOLUTIONS = [
  { value: '720', label: '1280 × 720' },
  { value: '1080', label: '1920 × 1080' },
  { value: '1440', label: '2560 × 1440' }
]

const wrap: React.CSSProperties = {
  // #root is overflow:hidden (HUD app shouldn't scroll), so the Showcase is its own
  // scroll container — full height with internal overflow.
  height: '100%',
  overflowY: 'auto',
  padding: 40,
  display: 'flex',
  flexDirection: 'column',
  gap: 36,
  background: '#0e0d10',
  color: 'var(--text)',
  fontFamily: 'var(--font-family)'
}
const row: React.CSSProperties = { display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }
const grid: React.CSSProperties = { display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(132px, 1fr))', gap: 12 }
const column: React.CSSProperties = { display: 'flex', flexDirection: 'column', gap: 16 }
const h: React.CSSProperties = {
  fontSize: 'var(--fs-xs)',
  textTransform: 'uppercase',
  letterSpacing: '.08em',
  color: 'var(--ink-45)',
  margin: '0 0 14px'
}

function Section({ title, children }: { title: string; children: React.ReactNode }): React.JSX.Element {
  return (
    <section>
      <p style={h}>{title}</p>
      {children}
    </section>
  )
}

function Swatch({ name, hex, token, dark }: { name: string; hex: string; token?: string; dark?: boolean }): React.JSX.Element {
  return (
    <div style={{ borderRadius: 'var(--r-card)', overflow: 'hidden', border: '1px solid var(--white-10)' }}>
      <div style={{ background: hex, height: 64, display: 'flex', alignItems: 'flex-end', padding: 8, color: dark ? '#161518' : '#fcfcfc', fontSize: 11, fontWeight: 700 }}>
        {hex.toUpperCase()}
      </div>
      <div style={{ padding: '8px 10px', background: 'var(--fill-2)' }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{name}</div>
        {token && <div style={{ fontSize: 11, color: 'var(--ink-45)', fontFamily: 'var(--font-mono)' }}>{token}</div>}
      </div>
    </div>
  )
}

/** Every section, by its name in ShowcaseIndex.ts. A section that keeps state renders a component. */
const RENDER: Record<ShowcaseName, () => React.JSX.Element> = {
  Palette: () => (
    <div style={grid}>
      {PALETTE.map((c) => (
        <Swatch key={c.name} {...c} />
      ))}
    </div>
  ),

  RarityColors: () => (
    <div style={grid}>
      {RARITY.map((c) => (
        <Swatch key={c.name} {...c} dark />
      ))}
    </div>
  ),

  Typography: () => (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
      {TYPE.map((t) => (
        <div key={t.name} style={{ display: 'flex', alignItems: 'baseline', gap: 16 }}>
          <span style={{ width: 96, fontSize: 11, color: 'var(--ink-45)', fontFamily: 'var(--font-mono)' }}>{t.token}</span>
          <span style={{ fontSize: `var(${t.token})`, fontWeight: t.weight }}>{t.name} — Decentraland</span>
        </div>
      ))}
    </div>
  ),

  Radii: () => (
    <div style={row}>
      {RADII.map((r) => (
        <div key={r.name} style={{ textAlign: 'center' }}>
          <div style={{ width: 72, height: 72, background: 'var(--fill-4)', borderRadius: `var(${r.token})`, border: '1px solid var(--white-10)' }} />
          <div style={{ fontSize: 11, marginTop: 6 }}>{r.name}</div>
          <div style={{ fontSize: 11, color: 'var(--ink-45)', fontFamily: 'var(--font-mono)' }}>{r.token}</div>
        </div>
      ))}
    </div>
  ),

  ArmedButton: () => <ArmedShowcase />,

  SignIn: () => <SignInShowcase />,

  LogoPattern: () => (
    <div style={{ position: 'relative', height: 160, background: 'var(--loading-vignette)', overflow: 'hidden' }}>
      <LogoPattern />
    </div>
  ),

  Dialogs: () => (
    <div style={row}>
      {/* Showcase renders instead of Hud, so this section hosts the popup layer its dialogs open into. */}
      <PopupHost />
      <Button appearance="hud"
        variant="secondary"
        onClick={() => {
          void showConfirm({
            title: 'Block Pablo?',
            body: "Blocked users won't be able to message you, join your community events, or see when you're online.",
            confirmLabel: 'Block'
          })
        }}
      >
        Confirm
      </Button>
      <Button appearance="hud"
        variant="secondary"
        onClick={() => {
          void showDialog({
            title: 'Report submitted',
            body: 'Thanks — our moderators will review this report.',
            actions: [{ id: 'ok', label: 'OK', variant: 'primary' }]
          })
        }}
      >
        Alert
      </Button>
      <Button appearance="hud"
        variant="secondary"
        onClick={() => {
          void showDialog({
            title: 'Unsaved changes',
            body: 'You have unsaved changes to your avatar.',
            width: 520,
            actions: [
              { id: 'cancel', label: 'Cancel', variant: 'ghost' },
              { id: 'discard', label: 'Discard', variant: 'secondary' },
              { id: 'save', label: 'Save', variant: 'primary' }
            ]
          })
        }}
      >
        Custom actions
      </Button>
    </div>
  ),

  Modal: () => (
    // The cards the popup layer centres, drawn in place (not modal here).
    <div style={{ ...row, alignItems: 'flex-start', gap: 24 }}>
      <ModalShell
        modal={false}
        title="Leave this community?"
        subtitle="You can join it again later."
        onClose={() => undefined}
        actionsEqual
        actions={
          <>
            <Button appearance="hud" variant="secondary">Cancel</Button>
            <Button appearance="hud">Leave</Button>
          </>
        }
      >
        <p style={{ margin: 0 }}>You stop getting its notifications and leave its chat.</p>
      </ModalShell>
      <Modal width={360} modal={false} ariaLabel="Report submitted">
        <ModalTitle title="Report submitted" onClose={() => undefined} />
        <p>Thanks — our moderators will review this report.</p>
        <ModalActions align="end">
          <Button appearance="hud">OK</Button>
        </ModalActions>
      </Modal>
    </div>
  ),

  ControlButton: () => (
    <div style={row}>
      <ControlButton aria-label="ghost square">✕</ControlButton>
      <ControlButton variant="solid" aria-label="solid square">✕</ControlButton>
      <ControlButton variant="dark" aria-label="dark square">✕</ControlButton>
      <ControlButton variant="faint" aria-label="faint square">✕</ControlButton>
      <ControlButton shape="circle" aria-label="circle">＋</ControlButton>
      <ControlButton shape="pill">＋ Pill</ControlButton>
      <ControlButton active aria-label="active">✓</ControlButton>
      <ControlButton size="sm" aria-label="small">✕</ControlButton>
      <ControlButton size="lg" aria-label="large">✕</ControlButton>
    </div>
  ),

  Tabs: () => <TabsShowcase />,

  FormControls: () => <FormControlsShowcase />,

  FieldLabel: () => <FieldLabelShowcase />,

  SearchField: () => <SearchFieldShowcase />,

  Dropdown: () => (
    <div style={{ width: 220, paddingBottom: 140 }}>
      <Dropdown options={['Recently listed', 'Cheapest', 'Newest']} defaultValue="Recently listed" />
    </div>
  ),

  EmoteSlot: () => (
    <div style={{ display: 'flex', gap: 24, alignItems: 'flex-start', flexWrap: 'wrap', padding: 16, background: '#3a3550', borderRadius: 12 }}>
      <EmoteSlot thumbnail={sampleEmote} rarity="legendary" width={220} />
      <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
        {EMOTE_RARITIES.map((r) => (
          <div key={r} style={{ textAlign: 'center' }}>
            <EmoteSlot thumbnail={sampleEmote} rarity={r} width={96} />
            <div style={{ fontSize: 11, color: 'var(--ink-7)', marginTop: 4, textTransform: 'capitalize' }}>{r}</div>
          </div>
        ))}
      </div>
    </div>
  ),

  HudScale: () => <HudScaleShowcase />,

  MenuPanel: () => (
    <MenuPanel label="Discover" title="DISCOVER">
      <MenuList>
        <MenuRow icon={<Icon name="events" size={24} />} label="Events" hint="X" trailing={<CountBadge count={3} />} />
        <MenuRow icon={<Icon name="places" size={24} />} label="Places" hint="Z" />
      </MenuList>
      <MenuDivider />
      <MenuList>
        <MenuRow icon={<Icon name="communities" size={24} />} label="Communities" hint="O" disabled />
      </MenuList>
    </MenuPanel>
  ),

  IconButton: () => (
    <Panel style={{ width: 60, padding: 8, display: 'flex', flexDirection: 'column', gap: 8 }}>
      {NAV.map((n, i) => (
        <IconButton key={n.icon} icon={n.icon} label={n.label} active={i === 8} badge={i === 1 ? 3 : undefined} />
      ))}
      <IconButton icon="profile" label="Profile (avatar)" avatar={{ name: 'Mojito', color: 'var(--lavender)' }} />
      <IconButton icon="voice-hearing" label="Voice (connected)" indicator />
      <IconButton icon="communities" label="Communities (live stream)" cornerArt={{ src: streamBadge, label: 'A community stream is live' }} />
    </Panel>
  ),

  Chip: () => (
    <>
      <div style={{ ...row, gap: 20 }}>
        <Chip label="All" selected onClick={() => undefined} />
        <Chip label="All" onClick={() => undefined} />
        <Chip label="Hair" selected onClick={() => undefined} onClear={() => undefined} />
      </div>
      <div style={{ ...row, gap: 6, marginTop: 12 }}>
        <Chip variant="dark" label="All" selected onClick={() => undefined} />
        <Chip variant="dark" label="Head" onClick={() => undefined} />
      </div>
    </>
  ),

  OptionMenu: () => (
    <OptionMenu
      onClose={() => undefined}
      sections={[
        { label: 'Sort by', options: [{ id: 'newest', label: 'Newest' }, { id: 'oldest', label: 'Oldest' }], value: 'newest', onChange: () => undefined },
        { label: 'View', options: [{ id: 'all', label: 'All Items' }, { id: 'col', label: 'Collectibles only' }], value: 'all', onChange: () => undefined }
      ]}
    />
  ),

  ContextMenu: () => (
    <ContextMenu
      items={[
        { label: 'View Community' },
        { kind: 'submenu', label: 'Notification Ping' },
        { kind: 'submenu', label: 'Invite to Community', items: [{ label: 'Pixel Builders' }, { label: 'Neon Club' }] },
        { kind: 'separator' },
        { kind: 'title', label: 'Notification Ping' },
        { kind: 'option', label: 'All Messages', checked: true },
        { kind: 'option', label: 'Mentions Only', checked: false },
        { kind: 'option', label: 'None', checked: false },
        { kind: 'separator' },
        { label: 'Delete chat history', danger: true }
      ]}
    />
  ),

  Submenu: () => (
    <div role="menu" aria-label="Community options" style={{ width: 240, padding: 6, paddingBottom: 96, borderRadius: 'var(--r-card)', background: 'var(--fill-2)' }}>
      <Submenu label="Invite to Community" icon={<People size={16} />}>
        <ContextMenu items={[{ label: 'Pixel Builders' }, { label: 'Neon Club' }, { label: 'Genesis Runners' }]} scroll />
      </Submenu>
    </div>
  ),

  AnchoredMenu: () => <AnchoredMenuDemo />,

  MaskIcon: () => (
    <div style={{ ...row, gap: 16 }}>
      {[reelShareIcon, reelLinkIcon, reelDownloadIcon, reelDeleteIcon].map((src) => (
        <MaskIcon key={src} src={src} size={24} />
      ))}
      <span style={{ ...row, color: 'var(--brand)' }}>
        <MaskIcon src={reelDeleteIcon} size={24} /> in the text colour
      </span>
    </div>
  ),

  KebabMenu: () => (
    <div style={{ display: 'flex', gap: 16, paddingBottom: 120 }}>
      <KebabMenu label="Conversation options" items={(close) => [{ label: 'View Community', onClick: close }, { label: 'Delete chat history', danger: true, onClick: close }]} />
      <KebabMenu size="sm" label="Participant options" items={(close) => [{ label: 'View profile', onClick: close }]} />
    </div>
  ),

  HintsButton: () => (
    <div style={{ ...row, gap: 24, paddingTop: 120 }}>
      <HintsButton hints={[{ icon: <KeyCap label="1" />, text: 'Press a number key to pick a slot.' }, { icon: <KeyCap label="E" />, text: 'Press E to open emotes.' }]} />
      <span style={{ ...row, gap: 6 }}>
        <KeyCap label="E" /> <KeyCap label="Esc" /> the key caps its hints show
      </span>
    </div>
  ),

  Rail: () => (
    <div style={{ width: 600 }}>
      <Rail perPage={3} gap={8}>
        {Array.from({ length: 8 }, (_, i) => (
          <div key={i} style={{ width: 194, height: 120, borderRadius: 'var(--lobby-card-radius)', background: 'var(--lobby-card)', display: 'grid', placeItems: 'center' }}>
            Card {i + 1}
          </div>
        ))}
      </Rail>
    </div>
  ),

  HeaderButton: () => (
    <>
      <HeaderButton aria-label="Close">
        <Close size={12} />
      </HeaderButton>
      <HeaderButton shape="pill" style={{ width: 163 }}>
        Profile
      </HeaderButton>
    </>
  ),

  VerifiedBadge: () => <VerifiedBadge />,

  Pager: () => (
    <>
      <Pager page={2} count={9} onChange={() => undefined} />
      <Pager variant="shop" page={0} count={9} onChange={() => undefined} />
    </>
  ),

  BrowsePage: () => <BrowsePageShowcase />,

  ColorPicker: () => (
    <div style={{ maxWidth: 320 }}>
      <ColorPicker label="Hair color" value="#5b310f" presets={['#1c1c1c', '#3c210b', '#5b310f', '#7b4818', '#985f37', '#8c2014', '#e98234', '#ffbe28', '#fad281', '#d4d4d4']} onChange={() => undefined} />
    </div>
  ),

  NameColorPicker: () => (
    <ColorPicker label="Name color" value="#40eaff" presets={['#a140ff', '#ff40ad', '#ff9840', '#ffe340', '#40ff45', '#40eaff']} hueOnly swatch onChange={() => undefined} />
  ),

  Brand: () => (
    <div style={{ ...row, gap: 16 }}>
      <DclLogo size={24} />
      <DclLogo size={32} />
      <DclLogo size={48} />
    </div>
  ),

  Avatar: () => (
    <div style={{ ...row, gap: 16 }}>
      <Avatar name="Mojito" color="#73d3d3" size={28} />
      <Avatar name="Pravus" color="#ff4bed" size={40} status="online" />
      <Avatar name="Mojito" size={40} status="online" src={samplePicture} />
      <Avatar name="Away" color="#a0abff" size={40} status="away" />
    </div>
  ),

  WearableCard: () => (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, 84px)', gap: 10 }}>
      {(['common', 'uncommon', 'rare', 'epic', 'legendary', 'mythic', 'unique', 'exotic', 'base'] as Rarity[]).map((r, i) => (
        <div key={r} style={{ width: 84 }}>
          <WearableCard rarity={r} equipped={i === 4} isNew={i === 1} />
        </div>
      ))}
    </div>
  ),

  WearableCardPick: () => (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, 136px)', gap: 10 }}>
      <WearableCard pick rarity="epic" count={3} />
      <WearableCard pick rarity="rare" selected />
      <WearableCard pick rarity="common" coverNote="Equipped" />
    </div>
  ),

  EquippedItemCard: () => (
    <>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(6, 108px)', gap: 12 }}>
        {(['common', 'uncommon', 'rare', 'epic', 'legendary', 'mythic', 'unique', 'exotic', 'base'] as Rarity[]).map((r, i) => (
          <EquippedItemCard
            key={r}
            name={`${r[0].toUpperCase()}${r.slice(1)} Item`}
            rarity={r}
            shopUrl={i % 2 === 0 ? 'https://decentraland.org/shop/item/0xa42e166edac870aa5351b098ae6458d39ca0fca6/0' : undefined}
            categoryIcon={<Icon name="emotes" size={15} />}
          />
        ))}
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(6, 108px)', gap: 12, marginTop: 12 }}>
        {(['epic', 'legendary', 'rare'] as Rarity[]).map((r, i) => (
          <EquippedItemCard
            key={r}
            name={`Pinned ${r}`}
            rarity={r}
            shopUrl="https://decentraland.org/shop/item/0xa42e166edac870aa5351b098ae6458d39ca0fca6/0"
            shopLabel={i === 0 ? 'Buy' : 'View'}
            price={i === 0 ? '25 Credits' : undefined}
            pinShop
            categoryIcon={<Icon name="emotes" size={15} />}
          />
        ))}
      </div>
    </>
  ),

  ScrollThumb: () => (
    <>
      <ShowcaseScroller />
      <ShowcaseRowScroller />
      <ShowcaseScroller tone="solid" />
    </>
  ),

  Toast: () => (
    <>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        <Button appearance="hud" variant="secondary" size="sm" onClick={() => showToast('User Mojito has been successfully blocked.')}>
          Plain
        </Button>
        <Button appearance="hud" variant="secondary" size="sm" onClick={() => showToast('Something went wrong.', { tone: 'error' })}>
          Error
        </Button>
        <Button appearance="hud"
          variant="secondary"
          size="sm"
          onClick={() => {
            let done = 0
            const tick = (): void => {
              done += 1
              showToast(`Optimizing textures… ${done} / 5`, { key: 'showcase-progress', duration: null, busy: true })
              if (done < 5) setTimeout(tick, 500)
              else showToast('Textures optimized · 4.2 MB → 1.1 MB — Reload to see them', { key: 'showcase-progress', duration: null, dismissible: true, action: { label: 'Reload', ariaLabel: 'Reload the scene', onClick: () => undefined } })
            }
            tick()
          }}
        >
          Keyed progress → action
        </Button>
      </div>
      <ToastHost />
    </>
  ),

  Chat: () => (
    <div style={{ display: 'flex', gap: 24, flexWrap: 'wrap', alignItems: 'flex-start' }}>
      <div style={{ width: 300, display: 'flex', flexDirection: 'column', gap: 8, padding: 12, background: 'rgba(19,19,19,0.6)', borderRadius: 14 }}>
        <DaySeparator ts={CHAT_TS} />
        <ChatBubble line={{ id: 1, sender: MOJITO, message: 'gm everyone 👋 welcome to the plaza', channel: 'Nearby', messageId: '', ts: CHAT_TS }} />
        <ChatBubble line={{ id: 2, sender: 'system', message: 'Type /help for available commands.', channel: 'System', messageId: '', ts: CHAT_TS }} />
      </div>
      <div style={{ width: 300, display: 'flex', flexDirection: 'column', gap: 2, padding: 8, background: 'rgba(12,11,14,0.97)', borderRadius: 14 }}>
        <MemberRow member={{ address: '0x5854cce95d5e25817b41f4c41f06b695a83bc495', name: 'Mojito', picture: samplePicture }} />
        <MemberRow member={{ address: '0x6723dcb07f3ca735223cd1c0acfa62dd994a1bb4', name: 'Sharknado#a1b2' }} />
        <MemberRow member={{ address: '0x1e105bb213754519903788022b962fe2b9c4b263', name: 'Pravus' }} />
      </div>
    </div>
  )
}

/** Every section with its render, in page order. */
export const SHOWCASE_SECTIONS = SHOWCASE_INDEX.map((section) => ({ ...section, render: RENDER[section.name] }))

export function Showcase(): React.JSX.Element {
  return (
    <div style={wrap}>
      <h1 style={{ margin: 0, fontSize: 'var(--fs-title)' }}>Design system · Explorer 2.0</h1>
      {/* Each render as an element, so its hooks and its failure belong to the section, not to the page. */}
      {SHOWCASE_SECTIONS.map(({ name, title, render: Render }) => (
        <Section key={name} title={title}>
          <Render />
        </Section>
      ))}
    </div>
  )
}

function AnchoredMenuDemo(): React.JSX.Element {
  const [anchor, setAnchor] = useState<{ x: number; y: number; opener: Element } | null>(null)
  const [pub, setPub] = useState(true)
  return (
    <>
      <Button appearance="hud"
        variant="secondary"
        onClick={(e) => {
          const box = e.currentTarget.getBoundingClientRect()
          setAnchor(anchor == null ? { x: box.left + box.width / 2, y: box.top + box.height / 2, opener: e.currentTarget } : null)
        }}
      >
        Open the photo options
      </Button>
      {anchor != null && (
        <AnchoredMenu
          anchor={anchor}
          ignore={anchor.opener}
          onClose={() => setAnchor(null)}
          aria-label="Photo options"
          items={[
            { kind: 'toggle', label: 'Set as Public', checked: pub, onChange: setPub },
            { kind: 'separator' },
            { label: 'Share on X', icon: <MaskIcon src={reelShareIcon} size={18} />, onClick: () => {} },
            { label: 'Copy Link', icon: <MaskIcon src={reelLinkIcon} size={18} />, onClick: () => {} },
            { label: 'Download', icon: <MaskIcon src={reelDownloadIcon} size={18} />, onClick: () => {} },
            { label: 'Delete', icon: <MaskIcon src={reelDeleteIcon} size={18} />, onClick: () => {} }
          ]}
        />
      )}
    </>
  )
}

function TabsShowcase(): React.JSX.Element {
  const [pillTab, setPillTab] = useState('wearables')
  const [lineTab, setLineTab] = useState('friends')
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 20, maxWidth: 420 }}>
      <Tabs
        aria-label="Pill example"
        value={pillTab}
        onChange={setPillTab}
        items={[
          { id: 'wearables', label: 'Wearables', icon: <Bag size={14} /> },
          { id: 'emotes', label: 'Emotes' },
          { id: 'outfits', label: 'Outfits', iconAfter: <Pin size={14} /> }
        ]}
      />
      <Tabs
        variant="underline"
        aria-label="Underline example"
        value={lineTab}
        onChange={setLineTab}
        items={[
          { id: 'friends', label: 'Friends', icon: <People size={14} /> },
          { id: 'requests', label: 'Requests', badge: 3 },
          { id: 'blocked', label: 'Blocked', disabled: true }
        ]}
      />
      <Tabs
        variant="shop"
        aria-label="Shop example"
        value={pillTab}
        onChange={setPillTab}
        items={[
          { id: 'wearables', label: 'Wearables', icon: <Bag size={18} /> },
          { id: 'emotes', label: 'Emotes' },
          { id: 'outfits', label: 'Outfits' }
        ]}
      />
    </div>
  )
}

function FormControlsShowcase(): React.JSX.Element {
  const [on, setOn] = useState(true)
  const [vol, setVol] = useState(60)
  const [res, setRes] = useState('1080')
  const [name, setName] = useState('robtfm')
  const [bio, setBio] = useState('')
  const [dob, setDob] = useState('2003-02-01')
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16, maxWidth: 320 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <span style={{ fontSize: 14 }}>Fullscreen</span>
        <Switch appearance="hud" checked={on} onCheckedChange={setOn} label="Fullscreen" labelHidden />
      </div>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: 8, borderRadius: 8, background: 'var(--text)', color: 'var(--panel)' }}>
        <span style={{ fontSize: 14 }}>On a light panel</span>
        <Switch appearance="hud" size="sm" surface="light" checked={on} onCheckedChange={setOn} label="On a light panel" labelHidden />
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <span style={{ fontSize: 14, width: 60 }}>Volume</span>
        <Slider value={vol} onChange={setVol} aria-label="Volume" />
        <span style={{ fontSize: 13, color: 'var(--ink-45)', width: 32 }}>{vol}</span>
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, height: 28 }}>
        <span style={{ fontSize: 14, width: 60 }}>Rail</span>
        <Slider variant="rail" value={vol} onChange={setVol} aria-label="Volume (navigation popover)" />
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <span style={{ fontSize: 14 }}>Display name</span>
        <TextInput value={name} onChange={setName} placeholder="Your name" maxLength={15} />
        <TextInput value={name} onChange={setName} variant="light" placeholder="light variant" maxLength={15} />
        <TextInput value="not-a-url" onChange={() => {}} invalid aria-label="Invalid example" />
        <TextInput value={name} onChange={setName} maxLength={15} aria-label="With an action" action={{ icon: 'dice', label: 'Random name', onClick: () => setName('RuneStarfall') }} />
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <span style={{ fontSize: 14 }}>About me</span>
        <TextArea value={bio} onChange={setBio} placeholder="Tell people about yourself" maxLength={400} counter />
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <span style={{ fontSize: 14 }}>Birth date (no native picker — see DateField)</span>
        <DateField label="Birth date" value={dob} onChange={setDob} />
      </div>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <span style={{ fontSize: 14 }}>Resolution</span>
        <Select value={res} onChange={setRes} aria-label="Resolution" options={RESOLUTIONS} />
      </div>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <span style={{ fontSize: 14 }}>Picker (placeholder)</span>
        <Select
          value=""
          onChange={() => undefined}
          aria-label="Add a place"
          placeholder="Select LAND or World"
          options={[
            { value: 'a', label: 'Genesis Plaza (0,0)' },
            { value: 'b', label: 'My World (example.dcl.eth)' }
          ]}
        />
      </div>
    </div>
  )
}

/** A labelled field with its sublabel and notice, and the counts CharCounter draws. */
function FieldLabelShowcase(): React.JSX.Element {
  const id = useId()
  const [status, setStatus] = useState('Building a museum')
  return (
    <div style={{ ...column, gap: 8, maxWidth: 320 }}>
      <FieldLabel htmlFor={id} sublabel="Shown on your passport" notice="Optional">
        Status
      </FieldLabel>
      <TextInput id={id} value={status} onChange={setStatus} maxLength={30} />
      <CharCounter current={status.length} max={30} />
      <div style={{ ...row, gap: 16 }}>
        <CharCounter current={12} max={40} format="spaced" />
        <CharCounter current={12} max={40} format="words" />
        <CharCounter current={52} max={40} />
      </div>
    </div>
  )
}

function SearchFieldShowcase(): React.JSX.Element {
  const [query, setQuery] = useState('plaza')
  return (
    <div style={{ ...column, gap: 12, maxWidth: 320 }}>
      <SearchField value={query} onChange={setQuery} placeholder="Search places" clearable />
      <SearchField variant="light" defaultValue="Genesis" placeholder="Search friends" />
      <SearchField size="sm" placeholder="Search emotes" />
    </div>
  )
}

/** The frame of the menu's browse pages (Places, Events, Shop): toolbar, controls, results panel. */
function BrowsePageShowcase(): React.JSX.Element {
  const [tab, setTab] = useState('wearables')
  const [query, setQuery] = useState('')
  return (
    <div style={{ maxWidth: 880, paddingBottom: 120 }}>
      <BrowseToolbar
        tabs={
          <Tabs
            aria-label="Shop sections"
            value={tab}
            onChange={setTab}
            items={[
              { id: 'wearables', label: 'Wearables' },
              { id: 'emotes', label: 'Emotes' },
              { id: 'names', label: 'Names' }
            ]}
          />
        }
      >
        <BrowseControl size="search">
          <SearchField value={query} onChange={setQuery} placeholder="Search the shop" />
        </BrowseControl>
        <BrowseControl size="select">
          <Dropdown options={['Recently listed', 'Cheapest', 'Newest']} defaultValue="Recently listed" />
        </BrowseControl>
      </BrowseToolbar>
      <div style={column}>
        <BrowsePanel label="Results, loading" busy>
          <BrowseLoading />
        </BrowsePanel>
        <BrowsePanel label="Results">
          <EmptyState variant="inline" title="Nothing on sale here" subtitle="Check back soon." />
        </BrowsePanel>
      </div>
    </div>
  )
}

/** The HUD scale against a few reference heights; the chosen one is outlined. */
function HudScaleShowcase(): React.JSX.Element {
  const [res, setRes] = useState('1080')
  const live = useSyncExternalStore(subscribeLiveScale, liveScale)
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 14, maxWidth: 560 }}>
      <p style={{ fontSize: 13, color: 'var(--ink-7)', margin: 0, lineHeight: 1.5 }}>
        The HUD scales by the smaller of width ÷ 1920 and height ÷ 1080. <code>innerHeight</code> is
        logical px (device px ÷ DPR), so a Retina panel scales as any display of the same logical size.
      </p>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <span style={{ fontSize: 14 }}>Reference resolution</span>
        <Select value={res} onChange={setRes} aria-label="Reference resolution" options={RESOLUTIONS} />
      </div>
      <div style={{ display: 'flex', gap: 20, flexWrap: 'wrap' }}>
        {['720', '900', '1080', '1440'].map((h) => {
          const s = Number(h) / 1080
          const sel = h === res
          return (
            <div key={h} style={{ textAlign: 'center' }}>
              <div style={{ fontSize: 12, color: sel ? 'var(--text)' : 'var(--ink-45)', marginBottom: 6, fontFamily: 'var(--font-mono)' }}>
                {h}p → ×{s.toFixed(3)}
              </div>
              <div style={{ width: 132, height: 84, display: 'flex', alignItems: 'flex-start' }}>
                <div
                  style={{
                    width: 100,
                    padding: '8px 10px',
                    borderRadius: 10,
                    background: 'var(--fill-4)',
                    border: sel ? '1px solid var(--brand)' : '1px solid var(--white-10)',
                    transform: `scale(${s})`,
                    transformOrigin: 'top left',
                    fontSize: 13,
                    fontWeight: 600
                  }}
                >
                  HUD card
                </div>
              </div>
            </div>
          )
        })}
      </div>
      <output aria-label="Live HUD scale" style={{ fontSize: 12, color: 'var(--ink-45)', fontFamily: 'var(--font-mono)' }}>
        {live}
      </output>
    </div>
  )
}

// The viewport and the `--ui-scale` the host installed for it (hudScale.ts, with its bounds), read again on every
// resize and every scale change, so the line shows what the HUD is drawn at.
function subscribeLiveScale(onChange: () => void): () => void {
  window.addEventListener('resize', onChange)
  const unsubscribe = subscribeHudScale(onChange)
  return () => {
    window.removeEventListener('resize', onChange)
    unsubscribe()
  }
}
const liveScale = (): string =>
  `Live viewport: ${window.innerWidth}×${window.innerHeight}px → ×${getHudScale().toFixed(3)}`

// A short scroller with a fixed-length thumb beside it.
function ShowcaseScroller({ tone }: { tone?: 'solid' }): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  return (
    <div style={{ position: 'relative', width: 440, height: 320 }}>
      <div ref={ref} style={{ height: '100%', overflowY: 'auto', scrollbarWidth: 'none', display: 'grid', gridTemplateColumns: 'repeat(2, 197px)', gap: 20 }}>
        {Array.from({ length: 6 }, (_, i) => (
          <div key={i} style={{ height: 200, borderRadius: 16, background: 'var(--white-10)' }} />
        ))}
      </div>
      <ScrollThumb target={ref} length={80} tone={tone} className={undefined} />
    </div>
  )
}

// A short row that scrolls sideways, its thumb along the bottom.
function ShowcaseRowScroller(): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  return (
    <div style={{ position: 'relative', width: 440, height: 140 }}>
      <div ref={ref} style={{ display: 'flex', gap: 16, overflowX: 'auto', scrollbarWidth: 'none' }}>
        {Array.from({ length: 6 }, (_, i) => (
          <div key={i} style={{ flex: 'none', width: 110, height: 110, borderRadius: 16, background: 'var(--white-10)' }} />
        ))}
      </div>
      <ScrollThumb target={ref} length={80} axis="x" className={undefined} />
    </div>
  )
}

/** ArmedButton mounted again by SHOW AGAIN, so its wait can be tried: a press before ARM_MS counts nothing. */
function ArmedShowcase(): React.JSX.Element {
  const [shown, setShown] = useState(0)
  const [sent, setSent] = useState(0)
  return (
    <div style={row}>
      <Button appearance="hud" variant="secondary" onClick={() => setShown((n) => n + 1)}>
        SHOW AGAIN
      </Button>
      <ArmedButton key={shown} onClick={() => setSent((n) => n + 1)}>
        SEND GIFT
      </ArmedButton>
      <span>
        Sent {sent} · answers {ARM_MS} ms after it appears
      </span>
    </div>
  )
}

function SignInShowcase(): React.JSX.Element {
  const [email, setEmail] = useState('')
  const [code, setCode] = useState('')
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16, width: 458, padding: 16, background: 'var(--loading-vignette)' }}>
      <SubmitField aria-label="Email" placeholder="your@email.com" value={email} onChange={setEmail} onSubmit={() => {}} submitLabel="Next" canSubmit={email.includes('@')} />
      <SubmitField aria-label="Email (invalid)" value="not-an-email" onChange={() => {}} onSubmit={() => {}} submitLabel="Next" canSubmit={false} error="Invalid email" />
      <Button appearance="hud" size="modal" variant="method">Google</Button>
      <Button appearance="hud" size="bare" variant="whisper" style={{ width: 36, height: 36 }} aria-label="whisper">‹</Button>
      <CodeInput value={code} onChange={setCode} />
      <CodeInput value="123456" onChange={() => {}} state="success" disabled />
      <CodeInput value="123456" onChange={() => {}} state="error" disabled />
      <Spinner size={40} variant="chase" />
    </div>
  )
}

