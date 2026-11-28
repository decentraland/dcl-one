// An avatar dressed in a set of items, in the wearable-preview iframe: the fitting room and the outfit pages, as
// decentraland.org's Shop draws them (shop@32377959 FittingRoom, OutfitPreview). The Bevy avatar preview where the
// `shop-bevy-avatar-preview` flag and the device allow it, then Unity where the `unity-wearable-preview` flag does,
// Babylon otherwise; a renderer that fails or stays silent is not asked for again for the rest of the stage's life.
// Covered until the preview has drawn what it was last sent.

import { useState, type ReactNode } from 'react'
import { frameRenderer, rendererOptions } from '../../../../shop/detail/lib/frameRenderer'
import PreviewFrame, {
  previewBase,
  UNITY_LOAD_TIMEOUT_MS,
  type PreviewFrameOptions
} from './PreviewFrame'

export type AvatarStageProps = {
  device: 'none' | 'full' | 'lite' | null
  unityFlag: boolean | null
  bevyFlag: boolean | null
  /** What to draw. The renderer is the stage's to pick. */
  options: Omit<PreviewFrameOptions, 'unity' | 'bevy' | 'unityMode'>
  /** Unity's mode, and Bevy's: `builder`, the default, draws a mannequin in exactly these items. */
  unityMode?: 'builder' | 'marketplace'
  /** Over the stage until the preview has drawn what it was last sent. */
  cover?: ReactNode
  /** In the preview's place when it cannot draw: no WebGL, or Babylon failed. The cover when absent. */
  unavailable?: ReactNode
}

export function AvatarStage({
  device,
  unityFlag,
  bevyFlag,
  options,
  unityMode = 'builder',
  cover = null,
  unavailable
}: AvatarStageProps) {
  const full = device === 'full'
  // A renderer failing sends the stage to the next one for the rest of its life.
  const [bevyFailed, setBevyFailed] = useState(false)
  const [unityFailed, setUnityFailed] = useState(false)

  const renderer =
    device === null || device === 'none'
      ? null
      : frameRenderer(
          full,
          { bevy: bevyFlag, unity: unityFlag },
          { bevy: bevyFailed, unity: unityFailed }
        )
  const settled = renderer !== null
  // The renderer and its mode are read once, from the mount URL: a new one is a new frame, drawn again from the start.
  const frame = `${renderer === 'babylon' || renderer === null ? 'babylon' : `${renderer}:${unityMode}`}|${previewBase(options.urns)}`
  const scene = `${frame}|${JSON.stringify(options)}`
  // Whether the preview has drawn or failed belongs to this visit to these options: coming back to options drawn or
  // failed before starts over, covered and mounted. Reset during render, so no frame shows the last visit's answer.
  const [visit, setVisit] = useState({ scene, drawn: false, failed: false })
  const current = visit.scene === scene ? visit : { scene, drawn: false, failed: false }
  if (current !== visit) setVisit(current)
  const answer = (change: { drawn: true } | { failed: true }) =>
    setVisit((v) => (v.scene === scene ? { ...v, ...change } : v))
  // A Babylon failure keeps the stage down for this visit only: the next change tries again.
  const mounted = settled && !current.failed
  const ready = mounted && current.drawn

  if (device === 'none') return <>{unavailable ?? cover}</>
  return (
    <>
      {mounted && renderer ? (
        <PreviewFrame
          key={frame}
          options={{ ...options, ...rendererOptions(renderer, unityMode) }}
          loadTimeoutMs={renderer === 'babylon' ? undefined : UNITY_LOAD_TIMEOUT_MS}
          onLoad={() => answer({ drawn: true })}
          // A fresh iframe has drawn nothing yet, even for the same items (Unity booting again).
          onReplaced={() => setVisit((v) => (v.scene === scene ? { ...v, drawn: false } : v))}
          onError={() =>
            renderer === 'bevy'
              ? setBevyFailed(true)
              : renderer === 'unity'
                ? setUnityFailed(true)
                : answer({ failed: true })
          }
        />
      ) : null}
      {ready ? null : settled && !mounted ? (unavailable ?? cover) : cover}
    </>
  )
}
