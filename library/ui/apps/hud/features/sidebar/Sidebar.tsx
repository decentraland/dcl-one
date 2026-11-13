// React HUD sidebar — replaces the SDK7 scene's nav rail. Matches the Explorer 2.0
// design: a 46px rail with a top group (nav/menus) and a bottom group (world tools
// + social). Chat toggles the React chat directly; the other items drive the
// scene's existing menus/popups over the bridge (session.nav) until each is
// migrated to React.

import { useCallback, useContext, useEffect, useState } from 'react'
import { Button, ControlButton, IconButton, Panel, Toggle } from '../../design'
import { toggleDebugPanel, useDebugPanelState } from '../debug/useDebugPanel'
import { useCommandResult } from '../debug/DebugPanel'
import type { IconName } from '../../design'
import type { NavAction } from '@explorer-core/engine/protocol'
import { keyHintFor, useBindingsSnapshot, type BindingsSnapshot } from '../../lib/bindingLabels'
import { userColor } from '@explorer-core/lib/identity'
import type { EngineSession } from '../session/useEngineSession'
import { EDITOR_BUILD } from '@explorer-core/features/editorHost/config'
import { EditorOffered } from '../editorHost/EditorOffered'
import { useLiveEventCount } from '../events/useLiveEventCount'
import { useAutoHide } from './useAutoHide'
import { hudInsetRef } from '../../lib/hudInset'
import styles from './Sidebar.module.css'
import notificationsArt from '../../assets/sidebar-rail/notifications.webp'
import eventsArt from '../../assets/sidebar-rail/events.webp'
import placesArt from '../../assets/sidebar-rail/places.webp'
import communitiesArt from '../../assets/sidebar-rail/communities.webp'
import backpackArt from '../../assets/sidebar-rail/backpack.webp'
import marketplaceArt from '../../assets/sidebar-rail/marketplace.webp'
import galleryArt from '../../assets/sidebar-rail/gallery.webp'
import settingsArt from '../../assets/sidebar-rail/settings.webp'
import helpArt from '../../assets/sidebar-rail/help.webp'
import bugArt from '../../assets/sidebar-rail/bug.webp'
import debugIcon from '../../assets/sidebar-icons/debug.png'
import skyboxArt from '../../assets/sidebar-rail/skybox.webp'
import emotesArt from '../../assets/sidebar-rail/emotes.webp'
import friendsArt from '../../assets/sidebar-rail/friends.webp'
import chatArt from '../../assets/sidebar-rail/chat.webp'
import voiceOffArt from '../../assets/sidebar-rail/voice-off.webp'
import voiceHearingArt from '../../assets/sidebar-rail/voice-hearing.webp'
import voiceSpeakingArt from '../../assets/sidebar-rail/voice-speaking.webp'
import { bugReportUrl } from '@explorer-core/lib/bugReport'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { NearbyVoiceWidget } from '../voice/NearbyVoiceWidget'
import { PermissionsPopover } from '../permissions/PermissionsPopover'
import { PortablesPopover, portablesTooltip, showPortables } from '../portables/PortablesPopover'
import { useNearbyVoice, type NearbyVoice } from '../voice/useNearbyVoice'
import { withUtm } from '@explorer-core/lib/utm'
import { IN_WORLD_SHOP, siteUrl } from '@hud-host'
import streamBadge from '../../assets/voice/stream-badge.png'

// `hotkey` names the engine SystemAction whose live binding renders as the tooltip hint.
type Item =
  | { kind: 'nav'; icon: IconName; label: string; action: NavAction; hotkey?: string }
  | { kind: 'chat'; icon: IconName; label: string; hotkey?: string }
  | { kind: 'friends'; icon: IconName; label: string; hotkey?: string }
  | { kind: 'emotes'; icon: IconName; label: string; hotkey?: string }
  | { kind: 'mic'; icon: IconName; label: string }
  | { kind: 'settings'; icon: IconName; label: string; hotkey?: string }
  | { kind: 'profile'; icon: IconName; label: string }
  | { kind: 'notifications'; icon: IconName; label: string }
  | { kind: 'backpack'; icon: IconName; label: string; hotkey?: string }
  | { kind: 'communities'; icon: IconName; label: string; hotkey?: string }
  | { kind: 'places'; icon: IconName; label: string; hotkey?: string }
  | { kind: 'events'; icon: IconName; label: string }
  | { kind: 'skybox'; icon: IconName; label: string }
  | { kind: 'permissions'; icon: IconName; label: string }
  | { kind: 'portables'; icon: IconName; label: string }
  | { kind: 'gallery'; icon: IconName; label: string; hotkey?: string }
  | { kind: 'shop'; icon: IconName; label: string }
  | { kind: 'camera'; icon: IconName; label: string; hotkey?: string }
  | { kind: 'create'; icon: IconName; label: string }
  | { kind: 'link'; icon: IconName; label: string; url: string | (() => string) }
  | { kind: 'divider' }


// The reference rail's own idle icons, each at the size it draws them.
const RAIL_ART: Partial<Record<IconName, { src: string; size: number; color?: boolean }>> = {
  'notifications': { src: notificationsArt, size: 30 },
  'events': { src: eventsArt, size: 32 },
  'places': { src: placesArt, size: 32 },
  'communities': { src: communitiesArt, size: 32 },
  'backpack': { src: backpackArt, size: 28 },
  'marketplace': { src: marketplaceArt, size: 32 },
  'gallery': { src: galleryArt, size: 28 },
  'settings': { src: settingsArt, size: 28 },
  'help': { src: helpArt, size: 32 },
  'bug': { src: bugArt, size: 30 },
  'skybox': { src: skyboxArt, size: 30 },
  'emotes': { src: emotesArt, size: 32 },
  'friends': { src: friendsArt, size: 30 },
  'chat': { src: chatArt, size: 26 },
  'voice-off': { src: voiceOffArt, size: 34, color: true },
  'voice-hearing': { src: voiceHearingArt, size: 34, color: true },
  'voice-speaking': { src: voiceSpeakingArt, size: 34, color: true }
}

function RailButton(props: React.ComponentProps<typeof IconButton>): React.JSX.Element {
  return <IconButton art={RAIL_ART[props.icon]} {...props} />
}

const TOP: Item[] = [
  { kind: 'profile', icon: 'profile', label: 'Profile' },
  { kind: 'notifications', icon: 'notifications', label: 'Notifications' },
  { kind: 'divider' },
  { kind: 'events', icon: 'events', label: 'Events' },
  { kind: 'places', icon: 'places', label: 'Places', hotkey: 'Places' },
  { kind: 'communities', icon: 'communities', label: 'Communities', hotkey: 'Communities' },
  { kind: 'backpack', icon: 'backpack', label: 'Backpack', hotkey: 'Backpack' },
  { kind: 'shop', icon: 'marketplace', label: 'Shop' },
  { kind: 'camera', icon: 'camera', label: 'Camera', hotkey: 'PhotoMode' },
  { kind: 'gallery', icon: 'gallery', label: 'Gallery', hotkey: 'Gallery' },
  { kind: 'settings', icon: 'settings', label: 'Settings', hotkey: 'Settings' },
  { kind: 'create', icon: 'create', label: 'Create' },
  { kind: 'divider' },
  { kind: 'link', icon: 'help', label: 'Help & Support', url: siteUrl('/help/') },
  { kind: 'link', icon: 'bug', label: 'Report a bug', url: bugReportUrl }
]

const BOTTOM: Item[] = [
  { kind: 'mic', icon: 'mic', label: 'Voice chat' },
  { kind: 'skybox', icon: 'skybox', label: 'Skybox' },
  { kind: 'permissions', icon: 'permissions', label: 'Scene permissions' },
  { kind: 'portables', icon: 'portables', label: 'Portable experiences' },
  { kind: 'divider' },
  { kind: 'emotes', icon: 'emotes', label: 'Emotes', hotkey: 'Emote' },
  { kind: 'divider' },
  { kind: 'friends', icon: 'friends', label: 'Friends', hotkey: 'Friends' },
  { kind: 'chat', icon: 'chat', label: 'Chat', hotkey: 'ChatPanel' }
]

type VoiceProps = NearbyVoice & { onOpen: (button: HTMLElement) => void }
type PopoverProps = { open: boolean; onOpen: (button: HTMLElement) => void }
type Popovers = { permissions: PopoverProps; portables: PopoverProps }

function renderItem(item: Item, i: number, session: EngineSession, snap: BindingsSnapshot, liveEvents: number, voice: VoiceProps, popovers: Popovers, create: boolean, onViewProfile?: () => void): React.JSX.Element | null {
  if (item.kind === 'divider') return <div key={`d${i}`} className={styles.divider} />
  const shortcut = 'hotkey' in item && item.hotkey != null ? keyHintFor(snap, item.hotkey) : undefined
  if (item.kind === 'chat')
    return (
      <RailButton
        key="chat"
        icon={item.icon}
        label={item.label}
        shortcut={shortcut}
        badge={session.chat.unread}
        active={session.chat.open}
        onClick={session.chat.toggle}
      />
    )
  if (item.kind === 'friends')
    return (
      <RailButton
        key="friends"
        icon={item.icon}
        label={item.label}
        shortcut={shortcut}
        badge={session.friends.received.length}
        badgeMax={9}
        active={session.friends.open}
        onClick={session.friends.toggle}
      />
    )
  if (item.kind === 'settings')
    return (
      <RailButton
        key="settings"
        icon={item.icon}
        label={item.label}
        shortcut={shortcut}
        active={session.settings.open}
        onClick={session.settings.toggle}
      />
    )
  if (item.kind === 'profile') {
    const p = session.profile.data
    return (
      <RailButton
        key="profile"
        icon={item.icon}
        avatar={p ? { src: p.picture, name: p.name, color: userColor(p.address, p.name, p.hasClaimedName, p.nameColor) } : undefined}
        label={item.label}
        active={session.profile.open}
        onClick={onViewProfile ?? session.profile.toggle}
      />
    )
  }
  if (item.kind === 'backpack')
    return (
      <RailButton
        key="backpack"
        icon={item.icon}
        label={item.label}
        shortcut={shortcut}
        active={session.backpack.open}
        onClick={session.backpack.toggle}
      />
    )
  if (item.kind === 'communities')
    return (
      <RailButton
        key="communities"
        icon={item.icon}
        label={item.label}
        shortcut={shortcut}
        active={session.communities.open}
        onClick={session.communities.toggle}
        cornerArt={session.voiceChat.active.some((c) => c.isMember) ? { src: streamBadge, label: 'A community stream is live' } : undefined}
      />
    )
  if (item.kind === 'places')
    return (
      <RailButton
        key="places"
        icon={item.icon}
        label={item.label}
        shortcut={shortcut}
        active={session.places.open}
        onClick={session.places.toggle}
      />
    )
  // The in-world Shop page where this HUD runs it (always, unless `?shop=old`); otherwise the web Shop in a new tab.
  if (item.kind === 'shop')
    return IN_WORLD_SHOP ? (
      <RailButton key="shop" icon={item.icon} label={item.label} active={session.shop.open} onClick={session.shop.toggle} />
    ) : (
      <RailButton key="shop" icon={item.icon} label={item.label} onClick={() => window.open(withUtm(siteUrl('/shop')), '_blank', 'noopener')} />
    )
  if (item.kind === 'camera')
    return (
      <RailButton key="camera" icon={item.icon} label={item.label} shortcut={shortcut} onClick={() => session.photoMode.set(true, 'button')} />
    )
  if (item.kind === 'gallery')
    return (
      <RailButton
        key="gallery"
        icon={item.icon}
        label={item.label}
        shortcut={shortcut}
        active={session.gallery.open}
        onClick={session.gallery.toggle}
      />
    )
  if (item.kind === 'notifications')
    return (
      <RailButton
        key="notifications"
        icon={item.icon}
        label={item.label}
        badge={session.notifications.unread}
        active={session.notifications.open}
        onClick={session.notifications.toggle}
      />
    )
  if (item.kind === 'emotes')
    return (
      <RailButton
        key="emotes"
        icon={item.icon}
        label={item.label}
        shortcut={shortcut}
        active={session.emotes.open}
        onClick={session.emotes.toggle}
      />
    )
  if (item.kind === 'mic') {
    const state = !voice.hearing ? 'off' : voice.speaking ? 'speaking' : 'hearing'
    return (
      <RailButton
        key="mic"
        icon={`voice-${state}`}
        size={34}
        label="Nearby Voice"
        data-voice={state}
        indicator={state !== 'off'}
        onClick={(e) => voice.onOpen(e.currentTarget)}
      >
        {state === 'speaking' && (
          <span className={styles.bars} data-live={voice.talking} aria-hidden="true">
            <i />
            <i />
            <i />
          </span>
        )}
      </RailButton>
    )
  }
  if (item.kind === 'create')
    return EDITOR_BUILD && create ? <RailButton key="create" icon={item.icon} label={item.label} active={session.create.open} onClick={session.create.toggle} /> : null
  if (item.kind === 'skybox')
    return <RailButton key="skybox" icon={item.icon} label={item.label} active={session.skybox.open} onClick={session.skybox.toggle} />
  if (item.kind === 'permissions') {
    // the badge counts refusals a stored rule decided that the player has not looked at
    return (
      <RailButton
        key="permissions"
        icon={item.icon}
        label={item.label}
        badge={session.permissions.unseenDenied}
        active={popovers.permissions.open}
        onClick={(e) => popovers.permissions.onOpen(e.currentTarget)}
      />
    )
  }
  if (item.kind === 'portables') {
    // only while the engine lists a portable (a smart wearable, or one a scene spawned)
    if (!showPortables(session.portables.list)) return null
    return (
      <RailButton
        key="portables"
        icon={item.icon}
        label={item.label}
        tooltip={portablesTooltip(session.portables.list)}
        active={popovers.portables.open}
        onClick={(e) => popovers.portables.onOpen(e.currentTarget)}
      />
    )
  }
  if (item.kind === 'events')
    return (
      <RailButton
        key="events"
        icon={item.icon}
        label={item.label}
        badge={liveEvents}
        badgeTone="lavender"
        active={session.events.open}
        onClick={session.events.toggle}
      />
    )
  if (item.kind === 'link')
    return (
      <RailButton
        key={item.label}
        icon={item.icon}
        label={item.label}
        onClick={() => window.open(typeof item.url === 'string' ? item.url : item.url(), '_blank', 'noopener')}
      />
    )
  return (
    <RailButton
      key={item.action}
      icon={item.icon}
      label={item.label}
      shortcut={shortcut}
      onClick={() => session.nav(item.action)}
    />
  )
}

function DotsGlyph(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 8" width="18" height="6" fill="currentColor" aria-hidden="true">
      <circle cx="4" cy="4" r="3" />
      <circle cx="12" cy="4" r="3" />
      <circle cx="20" cy="4" r="3" />
    </svg>
  )
}

export function Sidebar({
  session,
  onViewProfile
}: {
  session: EngineSession
  /** Open the local player's passport (the profile icon). */
  onViewProfile?: () => void
}): React.JSX.Element {
  const snap = useBindingsSnapshot()
  const liveEvents = useLiveEventCount()
  const create = useContext(EditorOffered)
  const [configOpen, setConfigOpen] = useState(false)
  const debugPanelOpen = useDebugPanelState() != null
  const { value: developmentBuild } = useCommandResult<boolean>(session.runDebugCommand, '/debug_panel --dev-build', false)
  const [voiceAnchor, setVoiceAnchor] = useState<HTMLElement | null>(null)
  const [permissionsAnchor, setPermissionsAnchor] = useState<HTMLElement | null>(null)
  const [portablesAnchor, setPortablesAnchor] = useState<HTMLElement | null>(null)
  const autoHide = useAutoHide(configOpen || voiceAnchor != null || permissionsAnchor != null || portablesAnchor != null)
  const nearby = useNearbyVoice(session)
  const voice: VoiceProps = { ...nearby, onOpen: (button) => setVoiceAnchor((a) => (a == null ? button : null)) }
  const closeVoice = useCallback(() => setVoiceAnchor(null), [])
  const popovers: Popovers = {
    permissions: {
      open: permissionsAnchor != null,
      onOpen: (button) => {
        session.permissions.markSeen()
        session.permissions.loadRules()
        setPermissionsAnchor((a) => (a == null ? button : null))
      }
    },
    portables: {
      open: portablesAnchor != null,
      onOpen: (button) => setPortablesAnchor((a) => (a == null ? button : null))
    }
  }
  const closePermissions = useCallback(() => setPermissionsAnchor(null), [])
  const closePortables = useCallback(() => setPortablesAnchor(null), [])
  // The button leaves the rail once the engine lists no portable; its popover goes with it.
  const portablesShown = showPortables(session.portables.list)
  useEffect(() => {
    if (!portablesShown) setPortablesAnchor(null)
  }, [portablesShown])
  // The "..." panel closes on an outside click or Cancel, like the other popovers.
  useEffect(() => {
    if (!configOpen) return
    const close = (): void => setConfigOpen(false)
    const onDown = (e: PointerEvent): void => {
      if (!(e.target instanceof Element && e.target.closest('[aria-label="Sidebar settings"]'))) close()
    }
    document.addEventListener('pointerdown', onDown, true)
    const off = registerCancelLayer(close)
    return () => {
      document.removeEventListener('pointerdown', onDown, true)
      off()
    }
  }, [configOpen])
  const launcher = developmentBuild === true && (
    <ControlButton variant="dark" className={styles.debugLauncher} aria-label="Toggle debug panel"
      title="Toggle debug panel" active={debugPanelOpen} onClick={() => toggleDebugPanel()}>
      <img src={debugIcon} width={28} height={28} alt="" draggable={false} />
    </ControlButton>
  )
  return (
    <>
      {launcher}
      {autoHide.hidden && <div className={styles.reveal} data-testid="sidebar-reveal" onPointerEnter={autoHide.onPointerEnter} />}
      <nav
        ref={hudInsetRef}
        className={styles.root}
        aria-label="Main navigation"
        data-hidden={autoHide.hidden}
        onPointerEnter={autoHide.onPointerEnter}
        onPointerLeave={autoHide.onPointerLeave}
      >
        <div className={styles.group}>
          <ControlButton
            size="sm"
            shape="pill"
            variant="solid"
            className={styles.configButton}
            aria-label="Sidebar settings"
            aria-expanded={configOpen}
            active={configOpen}
            onClick={() => setConfigOpen((o) => !o)}
          >
            <DotsGlyph />
          </ControlButton>
          {TOP.map((item, i) => renderItem(item, i, session, snap, liveEvents, voice, popovers, create, onViewProfile))}
        </div>
        <div className={styles.group}>{BOTTOM.map((item, i) => renderItem(item, i, session, snap, liveEvents, voice, popovers, create, onViewProfile))}</div>
      </nav>
      {voiceAnchor != null && (
        <NearbyVoiceWidget
          anchor={voiceAnchor}
          hearing={nearby.hearing}
          onHearingChange={(on) => {
            nearby.setHearing(on)
            if (!on) closeVoice()
          }}
          volume={nearby.volume}
          onVolumeChange={nearby.setVolume}
          speaking={nearby.speaking}
          talking={nearby.talking}
          micAvailable={nearby.micAvailable}
          onSpeakToggle={nearby.toggleSpeak}
          talkKey={keyHintFor(snap, 'Microphone')}
          onClose={closeVoice}
        />
      )}
      {permissionsAnchor != null && (
        <PermissionsPopover
          anchor={permissionsAnchor}
          visit={session.permissions.visit}
          triggered={session.permissions.triggered}
          rules={session.permissions.rules}
          sceneName={session.minimap.sceneTitle}
          setRule={session.permissions.setRule}
          clearRule={session.permissions.clearRule}
          resetDefaults={session.permissions.resetDefaults}
          onClose={closePermissions}
        />
      )}
      {portablesAnchor != null && (
        <PortablesPopover
          anchor={portablesAnchor}
          list={session.portables.list}
          onStop={session.portables.stop}
          onRestart={session.portables.restart}
          onClose={closePortables}
        />
      )}
      {configOpen && (
        <Panel className={styles.config} role="dialog" aria-label="Sidebar settings">
          <span>Auto-hide sidebar</span>
          <Toggle checked={autoHide.enabled} onChange={autoHide.setEnabled} aria-label="Auto-hide sidebar" />
          <Button size="sm" variant="secondary" onClick={() => {
            setConfigOpen(false)
            toggleDebugPanel()
          }}>Debug panel</Button>
        </Panel>
      )}
    </>
  )
}
