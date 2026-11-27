import { useEffect, useState } from 'react'
import {
  avatarShape as profileShape,
  BASE_MALE,
  itemShapes,
  type BodyShapeUrn
} from '../../../../shop/detail/lib/bodyShape'
import type { CatalogItem } from '../../../../shop/discovery/lib/catalogItem'
import { frameRenderer, rendererOptions } from '../../../../shop/detail/lib/frameRenderer'
import PreviewFrame, {
  previewBase,
  UNITY_LOAD_TIMEOUT_MS,
  type PreviewFrameOptions,
  type PreviewRendererName
} from './PreviewFrame'
import { PreviewSurface } from './PreviewControls'

type WebItemPreviewProps = {
  item: CatalogItem
  urn: string | null
  avatarShape: BodyShapeUrn | null
  fullDevice: boolean | null
  show3d: boolean
  onShow: () => void
  account: string | null
  ownBody?: string | null
  profileLoading: boolean
  unityFlag: boolean | null
  bevyFlag: boolean | null
}
// The web preview, as decentraland.org's Shop draws it (shop@3a322cf9 ItemPreview): the shopper's own avatar when it
// can wear the item, otherwise a default one of a shape the item fits; the item alone in the item view; an emote on the
// avatar. The Bevy avatar preview where the `shop-bevy-avatar-preview` flag and this device allow it, then the Unity
// renderer where the `unity-wearable-preview` flag does, Babylon otherwise; one that fails to load gives way to the next.
// Unity ships its own view and emote controls, as Bevy does after it, so the page draws its own only over Babylon.
export function WebItemPreview({
  item,
  urn,
  avatarShape,
  fullDevice,
  show3d,
  onShow,
  account,
  ownBody,
  profileLoading,
  unityFlag,
  bevyFlag
}: WebItemPreviewProps) {
  const [view, setView] = useState<'avatar' | 'item'>('avatar')
  // What the preview reported, each for the frame and scene it reported it on: a new frame or scene starts loading.
  const [drawn, setDrawn] = useState<{ scene: string; by: PreviewRendererName } | null>(null)
  const [failedFrame, setFailedFrame] = useState<string | null>(null)
  const [unityFailed, setUnityFailed] = useState(false)
  const [bevyFailed, setBevyFailed] = useState(false)
  const [generation, setGeneration] = useState(0)
  const [replay, setReplay] = useState(0)
  const canPreview = !!urn && item.category !== 'ens'
  const isEmote = item.category === 'emote'
  const wanted = canPreview && show3d
  const ownShape = ownBody ? profileShape({ avatar: { bodyShape: ownBody } }) : null
  const shopperShape = avatarShape ?? ownShape
  const shapes = itemShapes(item)
  const body =
    shopperShape && shapes.includes(shopperShape) ? shopperShape : (shapes[0] ?? BASE_MALE)
  const mismatch = !isEmote && !!shopperShape && !shapes.includes(shopperShape)
  const wearsOwn = !!ownShape && (isEmote || shapes.includes(ownShape))
  const itemAlone = !isEmote && view === 'item'
  // Mounted once with its final avatar and renderer, so the preview never loads a default and then the shopper's. A
  // lighter device draws with Babylon whatever the flags say, so it does not wait for them.
  const drawWith =
    fullDevice === null
      ? null
      : frameRenderer(
          fullDevice,
          { bevy: bevyFlag, unity: unityFlag },
          { bevy: bevyFailed, unity: unityFailed }
        )
  const settled = !(account && profileLoading) && drawWith !== null

  // A frame is mounted per renderer, preview origin, shopper and retry; a new item or view reloads its scene in place.
  const frame = [
    drawWith ?? 'babylon',
    previewBase(urn ? [urn] : []),
    account ?? '',
    generation
  ].join('|')
  const scene = `${frame}|${urn}|${itemAlone ? 'item' : 'avatar'}`
  const failed = failedFrame === frame
  const status: 'loading' | 'ready' | 'error' = failed
    ? 'error'
    : drawn?.scene === scene
      ? 'ready'
      : 'loading'
  const drawnBy = drawn?.scene === scene ? drawn.by : undefined
  const mounted = wanted && settled && !failed

  useEffect(() => {
    setView('avatar')
    setUnityFailed(false)
    setBevyFailed(false)
    setFailedFrame(null)
  }, [urn])
  // What a retired or replaced frame drew is not the next one's, even when the next one comes back under the same key.
  useEffect(() => {
    setDrawn(null)
  }, [mounted, frame])

  const options: PreviewFrameOptions = {
    urns: urn ? [urn] : [],
    profile: itemAlone ? undefined : wearsOwn ? account! : 'default',
    bodyShape: itemAlone || wearsOwn ? undefined : body,
    type: isEmote ? undefined : itemAlone ? 'wearable' : 'avatar',
    emote: isEmote || itemAlone ? undefined : 'fashion',
    disableBackground: true,
    // An emote's closer framing. Not for Bevy, which hides its own controls (its emote playback among them) wherever the
    // page frames the view itself, and the page's playbar is Babylon's only.
    wheelZoom: isEmote && drawWith !== 'bevy' ? 1.5 : undefined,
    wheelStart: isEmote && drawWith !== 'bevy' ? 100 : undefined,
    ...rendererOptions(drawWith ?? 'babylon', 'marketplace')
  }
  const ownControls = status === 'ready' && drawnBy === 'babylon'

  return (
    <PreviewSurface
      name={item.name}
      status={show3d ? status : 'thumbnail'}
      thumbnail={!mounted || status !== 'ready' ? item.thumbnail : undefined}
      overlay={mounted}
      loading={mounted && status === 'loading' && !item.thumbnail}
      unavailable={show3d && failed}
      view={ownControls && !isEmote ? { current: view, select: setView } : undefined}
      emote={ownControls && isEmote ? { replay, play: () => setReplay((n) => n + 1) } : undefined}
      show={!show3d && canPreview && fullDevice !== null ? onShow : undefined}
      shapeNote={mismatch && !itemAlone ? body : undefined}
      renderer={drawnBy}
    >
      {mounted && drawWith ? (
        <PreviewFrame
          key={frame}
          options={options}
          replay={replay}
          loadTimeoutMs={drawWith === 'babylon' ? undefined : UNITY_LOAD_TIMEOUT_MS}
          onLoad={(by) => setDrawn({ scene, by: by ?? drawWith })}
          // A fresh iframe has drawn nothing yet, even for the same scene (Unity booting again).
          onReplaced={() => setDrawn(null)}
          onError={() => {
            // The item alone failed where the avatar had drawn: back to the avatar, in a fresh frame. Before the
            // renderer: a preview that asked for Unity and drew with Babylon got this far on Babylon.
            if (itemAlone) {
              setView('avatar')
              setGeneration((n) => n + 1)
            } else if (drawWith === 'bevy') setBevyFailed(true)
            else if (drawWith === 'unity') setUnityFailed(true)
            else setFailedFrame(frame)
          }}
        />
      ) : null}
    </PreviewSurface>
  )
}
