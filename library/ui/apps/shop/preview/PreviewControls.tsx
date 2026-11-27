import type { ReactNode } from 'react'
import { t } from '../../../../shop/discovery/intl'
import { shapeLabel, type BodyShapeUrn } from '../../../../shop/detail/lib/bodyShape'
import styles from './ItemPreview.module.css'
const VIEW_TEXT = {
  previewMode: 'Preview mode',
  onAvatar: 'On avatar',
  itemView: 'Item',
  playEmote: 'Play emote'
}

function ViewToggle({
  view,
  onView
}: {
  view: 'avatar' | 'item'
  onView: (view: 'avatar' | 'item') => void
}) {
  return (
    <div
      className={styles.toggle}
      role="group"
      aria-label={VIEW_TEXT.previewMode}
      data-testid="preview-toggle"
    >
      <button
        type="button"
        data-active={view === 'avatar' || undefined}
        aria-pressed={view === 'avatar'}
        aria-label={VIEW_TEXT.onAvatar}
        onClick={() => onView('avatar')}
      >
        <span className={styles.toggleLabel}>{VIEW_TEXT.onAvatar}</span>
        <span className={styles.toggleIco} aria-hidden>
          <svg viewBox="0 0 20 20" width="18" height="18" fill="currentColor">
            <circle cx="10" cy="5" r="3" />
            <path d="M4 18c0-4 2.7-7 6-7s6 3 6 7z" />
          </svg>
        </span>
      </button>
      <button
        type="button"
        data-active={view === 'item' || undefined}
        aria-pressed={view === 'item'}
        aria-label={VIEW_TEXT.itemView}
        onClick={() => onView('item')}
      >
        <span className={styles.toggleLabel}>{VIEW_TEXT.itemView}</span>
        <span className={styles.toggleIco} aria-hidden>
          <svg viewBox="0 0 20 20" width="18" height="18" fill="currentColor">
            <path d="M7 3 3 5.5 4.5 9 6 8.3V17h8V8.3l1.5.7L17 5.5 13 3c-.5 1.3-1.6 2-3 2s-2.5-.7-3-2z" />
          </svg>
        </span>
      </button>
    </div>
  )
}

function EmotePlaybar({ replay, onReplay }: { replay: number; onReplay: () => void }) {
  return (
    <div className={styles.playbar} data-testid="emote-playbar">
      <button
        type="button"
        className={styles.play}
        onClick={onReplay}
        aria-label={VIEW_TEXT.playEmote}
      >
        <svg viewBox="0 0 20 20" width="16" height="16" fill="currentColor" aria-hidden>
          <path d="M6 4v12l10-6z" />
        </svg>
      </button>
      <span className={styles.track} aria-hidden>
        <span key={replay} className={styles.knob} />
      </span>
    </div>
  )
}

type View = 'avatar' | 'item'
type ControlsProps = {
  children: ReactNode
  status: string
  renderer?: string
  name: string
  thumbnail?: string | null
  overlay: boolean
  loading: boolean
  unavailable: boolean
  shapeNote?: BodyShapeUrn
  view?: { current: View; select: (view: View) => void }
  emote?: { replay: number; play: () => void }
  show?: () => void
}
export function PreviewSurface({
  children,
  status,
  renderer,
  name,
  thumbnail,
  overlay,
  loading,
  unavailable,
  shapeNote,
  view,
  emote,
  show
}: ControlsProps) {
  return (
    <div className={`shop-preview ${styles.root}`} data-status={status} data-renderer={renderer}>
      {children}
      {thumbnail && (
        <img
          className={styles.thumb}
          data-cover={overlay || undefined}
          src={thumbnail}
          alt={name}
        />
      )}
      {loading && (
        <div className={styles.loading} role="status">
          {t('itemDetail.previewLoading')}
        </div>
      )}
      {unavailable && (
        <p className={styles.note} role="status">
          {t('itemDetail.previewUnavailable')}
        </p>
      )}
      {view && <ViewToggle view={view.current} onView={view.select} />}
      {emote && <EmotePlaybar replay={emote.replay} onReplay={emote.play} />}
      {show && (
        <button type="button" className={styles.show} onClick={show} data-testid="show-3d">
          {t('itemDetail.showPreview')}
        </button>
      )}
      {shapeNote && (
        <p className={styles.note} data-testid="shape-note">
          {t('itemDetail.shownOnBody', { shape: shapeLabel(shapeNote) })}
        </p>
      )}
    </div>
  )
}
