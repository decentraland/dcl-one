// System scene for the React DOM HUD (react-web).
//
// It starts the bridge, which relays the engine's SystemApi to the React host over
// BroadcastChannel. Every HUD data source lives in exactly one `domains/*` file, so it's
// obvious where each piece comes from. The only things it renders are the two engine-backed
// views shown through transparent React cutouts — the Backpack's avatar preview and the
// minimap's Camera style; all other UI is React.

import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { startBridge } from './bridge'
import { registerAvatarPreview } from './domains/avatarPreview'
import { registerMinimap } from './domains/minimap'
import { renderSceneUi } from './ui'
import { registerSession } from './domains/session'
import { registerConnectivity } from './domains/connectivity'
import { registerAnalytics } from './domains/analytics'
import { registerProfile } from './domains/profile'
import { registerFriends } from './domains/friends'
import { registerChat } from './domains/chat'
import { registerDm } from './domains/dm'
import { registerUiSound } from './domains/uiSound'
import { registerEmotes } from './domains/emotes'
import { registerWearables, sendEquipped } from './domains/wearables'
import { registerCatalog } from './domains/catalog'
import { registerOutfits } from './domains/outfits'
import { registerRandomizer } from './domains/randomizer'
import { registerAvatarDraft } from './domains/avatarDraft'
import { registerWelcome } from './domains/welcome'
import { registerNotifications } from './domains/notifications'
import { registerSettings } from './domains/settings'
import { registerCommunities } from './domains/communities'
import { registerGallery } from './domains/gallery'
import { registerMapApi } from './domains/mapApi'
import { registerMapPins } from './domains/mapPins'
import { registerWorld } from './domains/world'
import { registerVoice } from './domains/voice'
import { registerVoiceChat } from './domains/voiceChat'
import { registerPointer } from './domains/pointer'
import { registerPhotoMode } from './domains/photoMode'
import { registerProximity } from './domains/proximity'
import { registerSystemAction } from './domains/systemAction'
import { registerBindings } from './domains/bindings'
import { registerAvatarPointer } from './domains/avatarPointer'
import { registerPermissions } from './domains/permissions'
import { registerPortables } from './domains/portables'
import { registerWorldAccess } from './domains/worldAccess'
import { registerEditor } from './domains/editor'
import { initNametags } from './domains/nametags'

export function main(): void {
  const _log = console.log
  console.log = (...args) => { _log('[System Scene]', ...args); }

  startBridge((ctx) => {
    registerSession(ctx)
    registerConnectivity(ctx)
    registerAnalytics(ctx)
    registerProfile(ctx)
    registerFriends(ctx)
    registerChat(ctx)
    registerDm(ctx)
    registerUiSound(ctx)
    registerVoice(ctx)
    registerVoiceChat(ctx)
    registerEmotes(ctx)
    registerWearables(ctx)
    registerCatalog(ctx)
    registerOutfits(ctx)
    registerRandomizer(ctx)
    registerAvatarDraft(ctx, async () => {
      await sendEquipped(ctx)
    })
    registerWelcome(ctx)
    registerNotifications(ctx)
    registerSettings(ctx)
    registerCommunities(ctx)
    registerGallery(ctx)
    registerMapApi(ctx)
    registerMapPins(ctx)
    registerWorld(ctx)
    registerPointer(ctx)
    registerProximity(ctx)
    registerPhotoMode(ctx)
    registerSystemAction(ctx)
    registerBindings(ctx)
    registerAvatarPointer(ctx)
    registerPermissions(ctx)
    registerPortables(ctx)
    registerWorldAccess(ctx)
    registerEditor(ctx)
    registerAvatarPreview(ctx)
    registerMinimap(ctx)
  })

  // A scene gets one UI renderer, so both cutout views are composed under a single root.
  //
  // No virtual screen (the documented `<= 0` opt-out) and no screen inset. By default the SDK
  // scales every raw `uiTransform` pixel by a contain-fit of a 1920x1080 design resolution
  // inside the canvas — right for a scene laid out in design pixels, exactly wrong for this
  // one. Everything here is positioned from a rect React measured on the real page, in canvas
  // pixels; scaling those a second time made the engine's minimap and avatar preview drift off
  // their React housings (the SDK's own scale is unclamped, the HUD's `--ui-scale` is clamped to
  // 0.6..1.3, so they only ever agreed near 1920x1080). `screenInset: 'none'` likewise keeps
  // 0,0 at the screen's top-left, which is the origin those rects are measured from.
  ReactEcsRenderer.setUiRenderer(renderSceneUi, { virtualWidth: 0, virtualHeight: 0, screenInset: 'none' })

  // World-space UI the DOM can't track smoothly: the billboarded nametag above each avatar's head.
  initNametags()
}
