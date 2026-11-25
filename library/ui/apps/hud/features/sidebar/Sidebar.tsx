// The HUD's navigation column: loose 42px circles down the left edge. The top group holds the
// "..." handle, Me, Map, Discover, Shop, System and the notifications bell; the bottom group holds
// the world tools (scene permissions, portable experiences, skybox), Volume, Emotes, Friends,
// Camera, Nearby Voice and Chat. Me, Discover, Shop, System, Volume and Camera each open a panel
// beside the column, one at a time; Map shows and hides the minimap.

import { useCallback, useContext, useEffect, useState } from 'react'
import { ControlButton, IconButton, MaskIcon, MenuList, MenuPanel, MenuRow, Panel, Slider, Toggle } from '../../design'
import { toggleDebugPanel, useDebugPanelState } from '../debug/useDebugPanel'
import { debugPanelLauncherEnabled } from '@explorer-core/lib/debugPanelLauncher'
import { keyHintFor, useBindingsSnapshot } from '../../lib/bindingLabels'
import { userColor } from '@explorer-core/lib/identity'
import { getPref, PREF, setPref } from '@explorer-core/lib/prefs'
import type { EngineSession } from '../session/useEngineSession'
import type { PassportTab } from '../profile/ProfilePassport'
import { EDITOR_BUILD } from '@explorer-core/features/editorHost/config'
import { EditorOffered } from '../editorHost/EditorOffered'
import { useLiveEventCount } from '../events/useLiveEventCount'
import { exitApplication } from '../gate/exitApplication'
import { signOutOfWorld } from '../gate/signOutOfWorld'
import { useAutoHide } from './useAutoHide'
import { useRailPopover } from './useRailPopover'
import { hudInsetRef } from '../../lib/hudInset'
import styles from './Sidebar.module.css'
import { boltArt } from './railIcons'
import { MePanel } from './panels/MePanel'
import { inviteLink } from './inviteLink'
import { MapPanel } from './panels/MapPanel'
import { DiscoverPanel } from './panels/DiscoverPanel'
import { ShopPanel } from './panels/ShopPanel'
import { SystemPanel } from './panels/SystemPanel'
import mapArt from '../../assets/sidebar-rail/map.svg?no-inline'
import mapOnArt from '../../assets/sidebar-rail/map-on.svg?no-inline'
import discoverArt from '../../assets/sidebar-rail/discover.svg?no-inline'
import discoverOnArt from '../../assets/sidebar-rail/discover-on.svg?no-inline'
import shopArt from '../../assets/sidebar-rail/shop.svg?no-inline'
import shopOnArt from '../../assets/sidebar-rail/shop-on.svg?no-inline'
import systemArt from '../../assets/sidebar-rail/system.svg?no-inline'
import systemOnArt from '../../assets/sidebar-rail/system-on.svg?no-inline'
import bellArt from '../../assets/sidebar-rail/bell.svg?no-inline'
import bellOnArt from '../../assets/sidebar-rail/bell-on.svg?no-inline'
import volumeArt from '../../assets/sidebar-rail/volume.svg?no-inline'
import friendsArt from '../../assets/sidebar-rail/friends.svg?no-inline'
import friendsOnArt from '../../assets/sidebar-rail/friends-on.svg?no-inline'
import cameraArt from '../../assets/sidebar-rail/camera.svg?no-inline'
import cameraOnArt from '../../assets/sidebar-rail/camera-on.svg?no-inline'
import cameraShotIcon from '../../assets/sidebar-rail/camera-shot.svg?no-inline'
import cameraGalleryIcon from '../../assets/sidebar-rail/camera-gallery.svg?no-inline'
import voiceArt from '../../assets/sidebar-rail/voice.svg?no-inline'
import voiceFaceArt from '../../assets/sidebar-rail/voice-face.svg?no-inline'
import chatArt from '../../assets/sidebar-rail/chat.svg?no-inline'
import chatOnArt from '../../assets/sidebar-rail/chat-on.svg?no-inline'
import skyboxArt from '../../assets/sidebar-rail/skybox.webp'
import emotesArt from '../../assets/sidebar-rail/emotes.webp'
import debugIcon from '../../assets/sidebar-icons/debug.png'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { NearbyVoiceWidget } from '../voice/NearbyVoiceWidget'
import { PermissionsPopover } from '../permissions/PermissionsPopover'
import { PortablesPopover, portablesTooltip, showPortables } from '../portables/PortablesPopover'
import { useNearbyVoice } from '../voice/useNearbyVoice'
import { withUtm } from '@explorer-core/lib/utm'
import { IN_WORLD_SHOP, isNativeHud, siteUrl } from '@hud-host'

/** A glyph of the design, drawn as it is in the 32px box inside the circle. */
const glyph = (src: string): { src: string; size: number; color: true } => ({ src, size: 32, color: true })

/** The panels the column opens beside itself, one at a time. */
type PanelId = 'me' | 'discover' | 'shop' | 'system' | 'volume' | 'camera'

/** How far above its button's top a panel's top lies, in reference px (the design's frames).
 *  Volume has none: its popover is centred on the button. */
const PANEL_RISE: Record<PanelId, number | undefined> = { me: 13, discover: 3, shop: 151, system: 3, volume: undefined, camera: 1 }

/** The panels that open over the place the minimap takes. */
const OVER_MAP: ReadonlySet<PanelId> = new Set<PanelId>(['me', 'discover', 'shop', 'system'])

/** The mock HUD (`?mock=1`: no engine, no session to shop with) draws the Shop panel on a host
 *  without the in-world Shop too, so the panel can be seen beside the bar. */
const mockHud = (): boolean => new URLSearchParams(location.search).get('mock') === '1'

/** The engine's name for the setting the Volume popover moves (`volume_settings.rs`: "<name> Volume"). */
const MASTER_VOLUME = 'Master Volume'

// A raster glyph's strokes are antialiased thin: a second copy laid over the first brings them up
// to solid white. The design's own glyphs are vectors and need none.
function RasterButton({ src, size, ...props }: Omit<React.ComponentProps<typeof IconButton>, 'art'> & { src: string; size: number }): React.JSX.Element {
  return (
    <IconButton art={{ src, size }} {...props}>
      <MaskIcon src={src} size={size} className={styles.overlay} />
    </IconButton>
  )
}

// The Map button closes the open panel itself (toggleMap) and decides from what shows when pressed.
const MAP_BUTTON = '[data-rail-map]'

/** Places what a button opens beside the column, and closes it on an outside press or Cancel. */
function PanelHost({ anchor, rise, onClose, children }: { anchor: HTMLElement; rise?: number; onClose: () => void; children: React.ReactNode }): React.JSX.Element {
  const { ref, style } = useRailPopover(anchor, onClose, rise, MAP_BUTTON)
  return (
    <div ref={ref} className={`${styles.panelHost} ${rise == null ? styles.centred : ''}`.trim()} style={style}>
      {children}
    </div>
  )
}

/** The Volume popover: the engine's master volume, when the engine lists it. */
function VolumePanel({ settings }: { settings: EngineSession['settings'] }): React.JSX.Element {
  const master = settings.list.find((s) => s.name === MASTER_VOLUME)
  return (
    <Panel role="dialog" aria-label="Volume" className={styles.volume}>
      <span className={styles.volumeLabel}>Master</span>
      {master != null && (
        <div className={styles.volumeSlider}>
          <Slider
            variant="rail"
            value={master.value}
            min={master.minValue}
            max={master.maxValue}
            step={master.stepSize || 1}
            onChange={(v) => settings.set(MASTER_VOLUME, v)}
            aria-label="Master"
          />
        </div>
      )}
    </Panel>
  )
}

const rowArt = (src: string): React.JSX.Element => <img src={src} alt="" width={28} height={28} draggable={false} />

function DotsGlyph(): React.JSX.Element {
  return (
    <svg viewBox="0 0 22.48 4.42" width="22.48" height="4.42" fill="currentColor" aria-hidden="true">
      <ellipse cx="2.3" cy="2.21" rx="2.3" ry="2.21" />
      <ellipse cx="11.24" cy="2.21" rx="2.3" ry="2.21" />
      <ellipse cx="20.18" cy="2.21" rx="2.3" ry="2.21" />
    </svg>
  )
}

export function Sidebar({
  session,
  onViewProfile
}: {
  session: EngineSession
  /** Open the local player's passport: the Me panel's Profile row, and its Badges row on that tab. */
  onViewProfile?: (tab?: PassportTab) => void
}): React.JSX.Element {
  const snap = useBindingsSnapshot()
  const liveEvents = useLiveEventCount()
  const create = useContext(EditorOffered)
  const [configOpen, setConfigOpen] = useState(false)
  const debugPanelOpen = useDebugPanelState() != null
  const [panel, setPanel] = useState<{ id: PanelId; anchor: HTMLElement } | null>(null)
  const [mapShown, setMapShown] = useState(() => getPref(PREF.sidebarMap) === '1')
  const [voiceAnchor, setVoiceAnchor] = useState<HTMLElement | null>(null)
  const [permissionsAnchor, setPermissionsAnchor] = useState<HTMLElement | null>(null)
  const [portablesAnchor, setPortablesAnchor] = useState<HTMLElement | null>(null)
  const autoHide = useAutoHide(configOpen || panel != null || voiceAnchor != null || permissionsAnchor != null || portablesAnchor != null)
  const nearby = useNearbyVoice(session)
  const closePanel = useCallback(() => setPanel(null), [])
  const closeVoice = useCallback(() => setVoiceAnchor(null), [])
  const closePermissions = useCallback(() => setPermissionsAnchor(null), [])
  const closePortables = useCallback(() => setPortablesAnchor(null), [])
  const isOpen = (id: PanelId): boolean => panel?.id === id
  const [shopPanel] = useState(() => IN_WORLD_SHOP || mockHud())
  // The notifications lie where the top panels and the minimap open, and no outside press closes
  // them: whichever of those opens next takes their place.
  const closeNotifications = (): void => {
    if (session.notifications.open) session.notifications.toggle()
  }
  const togglePanel = (id: PanelId) => (e: React.MouseEvent<HTMLElement>): void => {
    const anchor = e.currentTarget
    if (panel?.id !== id && OVER_MAP.has(id)) closeNotifications()
    setPanel((p) => (p?.id === id ? null : { id, anchor }))
  }
  // The minimap gives its place to a panel that opens over it, and to the notifications.
  const mapVisible = mapShown && !(panel != null && OVER_MAP.has(panel.id)) && !session.notifications.open
  const toggleMap = (): void => {
    const next = !mapVisible
    if (next) closeNotifications()
    setPanel(null)
    setMapShown(next)
    setPref(PREF.sidebarMap, next ? '1' : '0')
  }
  // The popover closes once the engine lists no portable.
  const portablesShown = showPortables(session.portables.list)
  useEffect(() => {
    if (!portablesShown) setPortablesAnchor(null)
  }, [portablesShown])
  // The "..." menu closes on an outside press or Cancel, like the panels.
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

  const profile = session.profile.data
  // Sign Out is for a sign-in on the web, a guest's included: as unity-explorer's, a guest that signs
  // out is kept, and PLAY AS GUEST brings it back. The desktop client cannot end its engine session
  // from the page, and a sign-in the scene owns is not the page's to end.
  const signOutOffered = session.login.signedInAs != null && !isNativeHud()
  const voiceState = nearby.blocked ? 'blocked' : !nearby.hearing ? 'off' : nearby.speaking ? 'speaking' : 'hearing'
  const voiceOn = voiceState === 'hearing' || voiceState === 'speaking'
  const launcher = debugPanelLauncherEnabled() && (
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
        data-autohide={autoHide.enabled}
        onPointerEnter={autoHide.onPointerEnter}
        onPointerLeave={autoHide.onPointerLeave}
      >
        <div className={`${styles.group} ${styles.top}`}>
          <ControlButton
            className={styles.handle}
            aria-label="Sidebar settings"
            aria-expanded={configOpen}
            active={configOpen}
            title="Sidebar settings"
            onClick={() => setConfigOpen((o) => !o)}
          >
            <span className={styles.handlePill}>
              <DotsGlyph />
            </span>
          </ControlButton>
          <IconButton
            icon="profile"
            avatar={profile ? { src: profile.picture, name: profile.name, color: userColor(profile.address, profile.name, profile.hasClaimedName, profile.nameColor) } : undefined}
            label="Me"
            active={isOpen('me')}
            quiet={isOpen('me')}
            onClick={togglePanel('me')}
          />
          <IconButton icon="map" art={glyph(mapVisible ? mapOnArt : mapArt)} label="Map" active={mapVisible} quiet={mapVisible} onClick={toggleMap} data-rail-map="" />
          <IconButton
            icon="places"
            art={glyph(isOpen('discover') ? discoverOnArt : discoverArt)}
            label="Discover"
            active={isOpen('discover')}
            quiet={isOpen('discover')}
            onClick={togglePanel('discover')}
          />
          {/* The Shop panel where this HUD runs the in-world Shop (always, unless `?shop=old`) and in the mock HUD; otherwise the web Shop in a new tab. */}
          {shopPanel ? (
            <IconButton icon="marketplace" art={glyph(isOpen('shop') ? shopOnArt : shopArt)} label="Shop" active={isOpen('shop')} quiet={isOpen('shop')} onClick={togglePanel('shop')} />
          ) : (
            <IconButton icon="marketplace" art={glyph(shopArt)} label="Shop" onClick={() => window.open(withUtm(siteUrl('/shop')), '_blank', 'noopener')} />
          )}
          <IconButton
            icon="settings"
            art={glyph(isOpen('system') ? systemOnArt : systemArt)}
            label="System"
            active={isOpen('system')}
            quiet={isOpen('system')}
            onClick={togglePanel('system')}
          />
          <IconButton
            icon="notifications"
            art={glyph(session.notifications.open ? bellOnArt : bellArt)}
            label="Notifications"
            badge={session.notifications.unread}
            active={session.notifications.open}
            quiet={session.notifications.open}
            onClick={session.notifications.toggle}
          />
        </div>
        <div className={styles.group}>
          {/* the badge counts refusals a stored rule decided that the player has not looked at */}
          <IconButton
            icon="permissions"
            label="Scene permissions"
            badge={session.permissions.unseenDenied}
            active={permissionsAnchor != null}
            quiet={permissionsAnchor != null}
            onClick={(e) => {
              const button = e.currentTarget
              session.permissions.markSeen()
              session.permissions.loadRules()
              setPermissionsAnchor((a) => (a == null ? button : null))
            }}
          />
          {/* in the column only while the engine lists a portable */}
          {portablesShown && (
            <IconButton
              icon="portables"
              art={glyph(boltArt)}
              label="Portable experiences"
              tooltip={portablesTooltip(session.portables.list)}
              active={portablesAnchor != null}
              quiet={portablesAnchor != null}
              onClick={(e) => {
                const button = e.currentTarget
                setPortablesAnchor((a) => (a == null ? button : null))
              }}
            />
          )}
          <RasterButton icon="skybox" src={skyboxArt} size={30} label="Skybox" active={session.skybox.open} quiet={session.skybox.open} onClick={session.skybox.toggle} />
          <IconButton
            icon="settings"
            art={glyph(volumeArt)}
            label="Volume"
            active={isOpen('volume')}
            quiet={isOpen('volume')}
            onClick={(e) => {
              // the engine sends its settings once asked
              session.settings.load()
              togglePanel('volume')(e)
            }}
          />
          <RasterButton icon="emotes" src={emotesArt} size={32} label="Emotes" shortcut={keyHintFor(snap, 'Emote')} active={session.emotes.open} onClick={session.emotes.toggle} />
          <IconButton
            icon="friends"
            art={glyph(session.friends.open ? friendsOnArt : friendsArt)}
            label="Friends"
            shortcut={keyHintFor(snap, 'Friends')}
            badge={session.friends.received.length}
            badgeMax={9}
            active={session.friends.open}
            quiet={session.friends.open}
            onClick={session.friends.toggle}
          />
          <IconButton
            icon="camera"
            art={glyph(isOpen('camera') ? cameraOnArt : cameraArt)}
            label="Camera"
            tooltip="Camera / Gallery"
            active={isOpen('camera')}
            quiet={isOpen('camera')}
            onClick={togglePanel('camera')}
          />
          <IconButton
            icon="mic"
            art={glyph(voiceState === 'speaking' ? voiceFaceArt : voiceArt)}
            className={voiceOn ? undefined : styles.voiceOff}
            label="Nearby Voice"
            data-voice={voiceState}
            indicator={voiceOn}
            active={voiceAnchor != null}
            quiet={voiceAnchor != null}
            onClick={(e) => {
              const button = e.currentTarget
              setVoiceAnchor((a) => (a == null ? button : null))
            }}
          >
            {voiceState === 'speaking' && (
              <span className={styles.bars} data-live={nearby.talking} aria-hidden="true">
                <i />
                <i />
                <i />
              </span>
            )}
          </IconButton>
          {/* the count shows only while the chat is closed */}
          <IconButton
            icon="chat"
            art={glyph(session.chat.open ? chatOnArt : chatArt)}
            label="Chat"
            shortcut={keyHintFor(snap, 'ChatPanel')}
            badge={session.chat.open ? undefined : session.chat.unread}
            active={session.chat.open}
            quiet={session.chat.open}
            onClick={session.chat.toggle}
          />
        </div>
      </nav>
      {mapVisible && <MapPanel minimap={session.minimap} map={session.map} referrer={profile?.address} setEngineViewport={session.setEngineViewport} />}
      {panel != null && (
        <PanelHost key={panel.id} anchor={panel.anchor} rise={PANEL_RISE[panel.id]} onClose={closePanel}>
          {panel.id === 'me' && (
            <MePanel
              profile={profile}
              backpackKey={keyHintFor(snap, 'Backpack')}
              inviteLink={inviteLink(profile)}
              onBackpack={session.backpack.toggle}
              onProfile={onViewProfile != null ? () => onViewProfile() : session.profile.toggle}
              onBadges={onViewProfile != null ? () => onViewProfile('badges') : undefined}
              onSignOut={signOutOffered ? signOutOfWorld : undefined}
              onExit={exitApplication}
              onClose={closePanel}
            />
          )}
          {panel.id === 'discover' && (
            <DiscoverPanel
              eventsKey={keyHintFor(snap, 'Events')}
              placesKey={keyHintFor(snap, 'Places')}
              communitiesKey={keyHintFor(snap, 'Communities')}
              liveEvents={liveEvents}
              communityStreamLive={session.voiceChat.active.some((c) => c.isMember)}
              onEvents={() => session.events.toggle('sidebar')}
              onPlaces={session.places.toggle}
              onCommunities={session.communities.toggle}
              onClose={closePanel}
            />
          )}
          {panel.id === 'shop' && <ShopPanel explore={{ wearables: session.shop.toggle }} onClose={closePanel} />}
          {panel.id === 'system' && (
            <SystemPanel
              settingsKey={keyHintFor(snap, 'Settings')}
              onSettings={session.settings.toggle}
              onCreate={EDITOR_BUILD && create ? session.create.toggle : undefined}
              onClose={closePanel}
            />
          )}
          {panel.id === 'volume' && <VolumePanel settings={session.settings} />}
          {panel.id === 'camera' && (
            <MenuPanel label="Camera" title="CAMERA">
              <MenuList>
                <MenuRow
                  icon={rowArt(cameraShotIcon)}
                  label="Take A Shot"
                  hint={keyHintFor(snap, 'PhotoMode')}
                  onClick={() => {
                    closePanel()
                    session.photoMode.set(true, 'button')
                  }}
                />
                <MenuRow
                  icon={rowArt(cameraGalleryIcon)}
                  label="Gallery"
                  hint={keyHintFor(snap, 'Gallery')}
                  onClick={() => {
                    closePanel()
                    session.gallery.toggle()
                  }}
                />
              </MenuList>
            </MenuPanel>
          )}
        </PanelHost>
      )}
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
          blocked={nearby.blocked}
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
          inForce={session.permissions.inForce}
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
      {configOpen && <div className={styles.configBackdrop} aria-hidden="true" />}
      {configOpen && (
        <Panel className={styles.config} role="dialog" aria-label="Sidebar settings">
          <div className={styles.configRow}>
            <span>Hide Sidebar Automatically</span>
            <Toggle size="sm" checked={autoHide.enabled} onChange={autoHide.setEnabled} aria-label="Hide Sidebar Automatically" />
          </div>
          {/* `/debug` in the chat opens the same panel. */}
          <button type="button" className={styles.configExtra} onClick={() => {
            setConfigOpen(false)
            toggleDebugPanel()
          }}>Debug panel</button>
        </Panel>
      )}
    </>
  )
}
