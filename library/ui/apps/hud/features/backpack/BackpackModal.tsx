// The Backpack as a modal over the lobby: the page's content in a centred frame over a dimmed
// lobby, with its own close button and no menu bar, so the lobby is still where you return to.

import { useRef } from 'react'
import { Close, HeaderButton } from '../../design'
import { useSession } from '../session/SessionContext'
import { holeClip } from '@explorer-core/lib/holeClip'
import styles from './BackpackModal.module.css'

export function BackpackModal({ onClose, children }: { onClose: () => void; children: React.ReactNode }): React.JSX.Element {
  const frameRef = useRef<HTMLDivElement>(null)
  const preview = useSession().avatarPreviewRect
  // the dim must not cover the engine's avatar, which shows through the frame's hole
  const hole = preview != null && frameRef.current != null ? modalHole(preview, frameRef.current) : null
  return (
    <div className={styles.root}>
      <div
        className={styles.dim}
        style={hole != null ? holeClip(hole.rect, hole.round) : undefined}
        onClick={(e) => {
          const frame = frameRef.current?.getBoundingClientRect()
          if (frame && e.clientX >= frame.left && e.clientX < frame.right && e.clientY >= frame.top && e.clientY < frame.bottom) return
          onClose()
        }}
        aria-hidden="true"
      />
      <div ref={frameRef} className={styles.frame} role="dialog" aria-label="Backpack">
        {children}
        <HeaderButton className={styles.close} aria-label="Close" onClick={onClose}>
          <Close size={12} />
        </HeaderButton>
      </div>
    </div>
  )
}

type Rect = { x: number; y: number; width: number; height: number }

// The layers under the modal open their hole a little wider than the page's own, so its
// anti-aliased edge blends into the engine's backdrop rather than the dark layers beneath.
const HOLE_BLEED = 2

/** The part of `rect` inside the modal frame (both in screen CSS px), widened by the bleed, or null. */
export function clip(rect: Rect, frame: DOMRect): Rect | null {
  const x = Math.max(rect.x - HOLE_BLEED, frame.left)
  const y = Math.max(rect.y - HOLE_BLEED, frame.top)
  const width = Math.min(rect.x + rect.width + HOLE_BLEED, frame.right) - x
  const height = Math.min(rect.y + rect.height + HOLE_BLEED, frame.bottom) - y
  return width > 0 && height > 0 ? { x, y, width, height } : null
}

/** The hole a layer under the modal opens for `rect`: the part inside the frame, rounded where it
 *  reaches one of the frame's own rounded corners. */
export function modalHole(rect: Rect, frameEl: HTMLElement): { rect: Rect; round: { topLeft: number; topRight: number; bottomRight: number; bottomLeft: number } } | null {
  const frame = frameEl.getBoundingClientRect()
  const hole = clip(rect, frame)
  if (hole == null) return null
  const radius = (parseFloat(getComputedStyle(frameEl).borderTopLeftRadius) || 0) * (frame.width / (frameEl.offsetWidth || frame.width || 1))
  const left = hole.x <= frame.left
  const top = hole.y <= frame.top
  const right = hole.x + hole.width >= frame.right
  const bottom = hole.y + hole.height >= frame.bottom
  return {
    rect: hole,
    round: {
      topLeft: left && top ? radius : 0,
      topRight: right && top ? radius : 0,
      bottomRight: right && bottom ? radius : 0,
      bottomLeft: left && bottom ? radius : 0
    }
  }
}
