// The lobby's layout editor: a frame with a remove button over every placed widget, dragged to move it
// (the widgets in its way make room), and the "Customize Lobby" modal listing the widgets off the
// grids, dragged onto one to place them (or dropped back on the modal to take them off). Save stores
// the layout; Cancel, Back and Escape drop the changes; Reset to Default puts the shipped layout up
// without storing it.

import { useCallback, useEffect, useRef, useState } from 'react'
import closeIcon from '../../assets/lobby/close-icn-thin.png'
import checkmark from '../../assets/lobby/checkmark-icn-thick.png'
import communitiesIcon from '../../assets/lobby/communities-icon.png'
import dragIcon from '../../assets/lobby/drag-icon.png'
import eventsIcon from '../../assets/lobby/events-icon.png'
import friendsIcon from '../../assets/lobby/friends-icon.png'
import galleryIcon from '../../assets/lobby/gallery-icon.png'
import goToIcon from '../../assets/lobby/go-to-icon.png'
import homeIcon from '../../assets/lobby/home-icon.png'
import jumpBackIcon from '../../assets/lobby/jump-back-icon.png'
import liveIcon from '../../assets/lobby/live-icon.png'
import recommendedIcon from '../../assets/lobby/recommended-icon.png'
import { MaskIcon } from '../../design'
import {
  LOBBY_CORNERS,
  LOBBY_FAMILIES,
  LobbyLayout,
  familyTitle,
  familyVariants,
  widgetCardCount,
  widgetFamily,
  widgetSpan,
  type Cell,
  type LobbyCorner,
  type LobbyWidgetFamily,
  type LobbyWidgetId
} from '@explorer-core/features/lobby/layout'
import { cellRect, gridAt, gridRect, nearestCell, spanSize, CELL_SIZE, GUTTER, type Rect } from './grid'
import styles from './LobbyEditor.module.css'

const FAMILY_ICONS: Record<LobbyWidgetFamily, string> = {
  Landing: homeIcon,
  LiveEvents: liveIcon,
  UpcomingEvents: eventsIcon,
  FriendsOnline: friendsIcon,
  JumpBackIn: jumpBackIcon,
  RecommendedPlaces: recommendedIcon,
  Communities: communitiesIcon,
  Gallery: galleryIcon,
  GoTo: goToIcon
}

/** A family's sizes as the modal offers them, fewest cards first (the first is the one it starts on). */
const sizesOf = (family: LobbyWidgetFamily): LobbyWidgetId[] => [...familyVariants(family)].sort((a, b) => widgetCardCount(a) - widgetCardCount(b))

type Canvas = { width: number; height: number }

interface Drag {
  widget: LobbyWidgetId
  from: 'grid' | 'modal'
  pointer: number
  /** The pointer's offset from the dragged widget's top-left corner. */
  grab: { x: number; y: number }
  /** The dragged widget's top-left corner on the canvas. */
  at: { x: number; y: number }
}

export interface DropTarget {
  corner: LobbyCorner
  cell: Cell
  valid: boolean
}

/** What a drag in progress would do. */
export function dropTarget(layout: LobbyLayout, drag: Drag, canvas: Canvas): DropTarget | null {
  const span = widgetSpan(drag.widget)
  const centre = { x: drag.at.x + spanSize(span.x) / 2, y: drag.at.y + spanSize(span.y) / 2 }
  const corner = gridAt(centre, canvas)
  if (corner == null) return null
  const grid = gridRect(corner, canvas)
  const cell = nearestCell(drag.at.x - grid.x, drag.at.y - grid.y)
  return { corner, cell, valid: layout.resolvePush(drag.widget, corner, cell) != null }
}

export interface LayoutEditor {
  /** The layout the page draws while editing: the draft, or where a drag would leave it. */
  display: LobbyLayout
  /** The widget being dragged and where; the page draws its stand-in there. */
  lifted: { widget: LobbyWidgetId; rect: Rect } | null
  changed: boolean
  draft: LobbyLayout
}

/** The editor's state: the draft layout, the drag in progress, and what each would draw. */
export function useLayoutEditor(saved: LobbyLayout, canvas: Canvas, toCanvas: (clientX: number, clientY: number) => { x: number; y: number }) {
  const [draft, setDraft] = useState(() => saved.clone())
  const [drag, setDrag] = useState<Drag | null>(null)
  const [overModal, setOverModal] = useState(false)
  const modal = useRef<HTMLDivElement>(null)
  const dragRef = useRef(drag)
  dragRef.current = drag

  const target = drag != null ? dropTarget(draft, drag, canvas) : null
  let display = draft
  if (drag != null) {
    display = draft.clone()
    display.remove(drag.widget)
    if (target?.valid) {
      const preview = draft.clone()
      preview.tryPlacePushing(drag.widget, target.corner, target.cell)
      preview.remove(drag.widget)
      display = preview
    }
  }

  const canvasRef = useRef(canvas)
  canvasRef.current = canvas
  const listeners = useRef<(() => void) | null>(null)
  useEffect(() => () => listeners.current?.(), [])

  // The drag is followed from its press: listening from an effect would miss a release that comes first.
  const start = useCallback(
    (widget: LobbyWidgetId, from: 'grid' | 'modal', e: React.PointerEvent, rect: Rect) => {
      if (e.button !== 0) return
      e.preventDefault()
      const p = toCanvas(e.clientX, e.clientY)
      // a stand-in from the modal hangs from its centre; a placed widget keeps where it was grabbed
      const grab = from === 'modal' ? { x: rect.width / 2, y: rect.height / 2 } : { x: p.x - rect.x, y: p.y - rect.y }
      const first: Drag = { widget, from, pointer: e.pointerId, grab, at: { x: p.x - grab.x, y: p.y - grab.y } }
      dragRef.current = first
      setDrag(first)
      listeners.current?.()
      const overTheModal = (ev: PointerEvent): boolean => {
        const m = modal.current?.getBoundingClientRect()
        return from === 'grid' && m != null && ev.clientX >= m.left && ev.clientX <= m.right && ev.clientY >= m.top && ev.clientY <= m.bottom
      }
      const move = (ev: PointerEvent): void => {
        const d = dragRef.current
        if (d == null || ev.pointerId !== d.pointer) return
        const at = toCanvas(ev.clientX, ev.clientY)
        const next = { ...d, at: { x: at.x - d.grab.x, y: at.y - d.grab.y } }
        dragRef.current = next
        setDrag(next)
        setOverModal(overTheModal(ev))
      }
      const stop = (): void => {
        window.removeEventListener('pointermove', move)
        window.removeEventListener('pointerup', up)
        window.removeEventListener('pointercancel', cancel)
        listeners.current = null
        dragRef.current = null
        setDrag(null)
        setOverModal(false)
      }
      const up = (ev: PointerEvent): void => {
        const d = dragRef.current
        if (d == null || ev.pointerId !== d.pointer) return
        const at = toCanvas(ev.clientX, ev.clientY)
        const last = { ...d, at: { x: at.x - d.grab.x, y: at.y - d.grab.y } }
        const onModal = overTheModal(ev)
        setDraft((current) => {
          const next = current.clone()
          if (onModal) next.remove(last.widget)
          else {
            const t = dropTarget(current, last, canvasRef.current)
            if (t?.valid) next.tryPlacePushing(last.widget, t.corner, t.cell)
          }
          return next
        })
        stop()
      }
      const cancel = (ev: PointerEvent): void => {
        if (ev.pointerId === dragRef.current?.pointer) stop()
      }
      window.addEventListener('pointermove', move)
      window.addEventListener('pointerup', up)
      window.addEventListener('pointercancel', cancel)
      listeners.current = stop
    },
    [toCanvas]
  )

  const lifted = drag == null ? null : { widget: drag.widget, rect: { x: drag.at.x, y: drag.at.y, width: spanSize(widgetSpan(drag.widget).x), height: spanSize(widgetSpan(drag.widget).y) } }

  return {
    editor: { display, lifted, changed: !draft.equals(saved), draft } satisfies LayoutEditor,
    drag,
    target,
    overModal,
    modal,
    start,
    remove: (widget: LobbyWidgetId) =>
      setDraft((current) => {
        const next = current.clone()
        next.remove(widget)
        return next
      }),
    reset: () =>
      setDraft(() => {
        const next = saved.clone()
        next.resetToDefault()
        return next
      }),
    /** Place a widget from the modal without dragging: the first free cell of the first grid with room. */
    placeAnywhere: (widget: LobbyWidgetId) =>
      setDraft((current) => {
        const next = current.clone()
        for (const corner of LOBBY_CORNERS) {
          const cell = next.findSnapCell(corner, widgetSpan(widget), { x: 0, y: 0 })
          if (cell != null && next.tryPlace(widget, corner, cell)) break
        }
        return next
      })
  }
}

/** The grids' backdrops with their anchor dots, the dot colour showing whether a drop there takes. */
export function EditorGrids({ canvas, target, size }: { canvas: Canvas; target: DropTarget | null; size: LobbyWidgetId | null }): React.JSX.Element {
  return (
    <>
      {LOBBY_CORNERS.map((corner) => {
        const r = gridRect(corner, canvas)
        const cols = Math.round((r.width + GUTTER) / (CELL_SIZE + GUTTER))
        const rows = Math.round((r.height + GUTTER) / (CELL_SIZE + GUTTER))
        const hovered = target?.corner === corner
        const span = size != null ? widgetSpan(size) : null
        const inDrop = (x: number, y: number): boolean =>
          hovered && span != null && target != null && x >= target.cell.x && x < target.cell.x + span.x && y >= target.cell.y && y < target.cell.y + span.y
        return (
          <div key={corner} className={`${styles.grid} ${hovered ? styles.gridHighlighted : ''}`.trim()} style={{ left: r.x, top: r.y, width: r.width, height: r.height }} aria-hidden="true">
            <span className={styles.gridBackdrop} />
            {Array.from({ length: rows }, (_, y) =>
              Array.from({ length: cols }, (_, x) => (
                <span
                  key={`${x},${y}`}
                  className={`${styles.cellDots} ${inDrop(x, y) ? (target?.valid ? styles.dotsValid : styles.dotsInvalid) : ''}`.trim()}
                  style={{ left: x * (CELL_SIZE + GUTTER), top: y * (CELL_SIZE + GUTTER) }}
                />
              ))
            )}
          </div>
        )
      })}
    </>
  )
}

/** The frame over a placed widget: dragged to move it, with a button that takes it off its grid. */
export function WidgetHandle({
  widget,
  rect,
  onStart,
  onRemove
}: {
  widget: LobbyWidgetId
  rect: Rect
  onStart: (e: React.PointerEvent) => void
  onRemove: () => void
}): React.JSX.Element {
  const title = familyTitle(widgetFamily(widget))
  return (
    <div className={styles.handle} style={{ left: rect.x, top: rect.y, width: rect.width, height: rect.height }} onPointerDown={onStart} role="group" aria-label={`${title} widget`}>
      <span className={styles.underlay} />
      <button
        type="button"
        className={styles.remove}
        aria-label={`Remove ${title}`}
        onPointerDown={(e) => e.stopPropagation()}
        onClick={onRemove}
      >
        <MaskIcon src={closeIcon} size={12} />
      </button>
    </div>
  )
}

/** A widget's drop marker under the cells it would take. */
export function DropMarker({ target, widget, canvas }: { target: DropTarget; widget: LobbyWidgetId; canvas: Canvas }): React.JSX.Element {
  const r = cellRect(target.corner, target.cell, widgetSpan(widget), canvas)
  return <div className={`${styles.marker} ${target.valid ? styles.markerValid : styles.markerInvalid}`} style={{ left: r.x, top: r.y, width: r.width, height: r.height }} aria-hidden="true" />
}

/** The stand-in a dragged widget shows: its title over blank cards and a pager. */
export function WidgetPreview({ widget, rect, welcome }: { widget: LobbyWidgetId; rect: Rect; welcome: string }): React.JSX.Element {
  const family = widgetFamily(widget)
  const narrow = widget.endsWith('4By2')
  const kind =
    family === 'Landing' ? 'landing' : family === 'LiveEvents' ? 'live' : family === 'UpcomingEvents' ? 'upcoming' : family === 'FriendsOnline' ? 'friends' : family === 'GoTo' ? 'goto' : family === 'Gallery' ? 'gallery' : 'places'
  const title = family === 'Landing' ? welcome : family === 'GoTo' ? 'Jump to any Genesis City coordinate or a World' : familyTitle(family)
  const cards = family === 'Landing' || family === 'GoTo' ? 1 : widgetCardCount(widget)
  const pager = !(family === 'Landing' || family === 'GoTo' || family === 'JumpBackIn')
  return (
    <div className={`${styles.preview} ${styles[`preview_${kind}`]} ${narrow ? styles.previewNarrow : ''}`} style={{ left: rect.x, top: rect.y, width: rect.width, height: rect.height }} aria-hidden="true">
      <span className={styles.previewFrame} />
      <p className={styles.previewTitle}>{title}</p>
      <div className={styles.previewCards}>
        {Array.from({ length: cards }, (_, i) => (
          <span key={i} className={styles.previewCard} />
        ))}
      </div>
      {pager && (
        <div className={styles.previewPager}>
          <span className={`${styles.previewDot} ${styles.previewDotSelected}`} />
          <span className={styles.previewDot} />
          <span className={styles.previewDot} />
        </div>
      )}
    </div>
  )
}

/** The "Customize Lobby" modal: the widgets off the grids, and the layout's Save, Cancel and Reset. */
export function CustomizeModal({
  layout,
  target,
  modalRef,
  dragging,
  changed,
  onStart,
  onPlace,
  onReset,
  onCancel,
  onSave
}: {
  layout: LobbyLayout
  target: boolean
  modalRef: React.Ref<HTMLDivElement>
  dragging: LobbyWidgetId | null
  changed: boolean
  onStart: (widget: LobbyWidgetId, e: React.PointerEvent, size: { width: number; height: number }) => void
  onPlace: (widget: LobbyWidgetId) => void
  onReset: () => void
  onCancel: () => void
  onSave: () => void
}): React.JSX.Element {
  const [sizes, setSizes] = useState<Partial<Record<LobbyWidgetFamily, LobbyWidgetId>>>({})
  const offGrid = LOBBY_FAMILIES.filter((f) => !familyVariants(f).some((w) => layout.placement(w) != null))
  return (
    <div ref={modalRef} className={`${styles.modal} ${target ? styles.modalTarget : ''}`.trim()} role="dialog" aria-label="Customize Lobby">
      <h2 className={styles.modalTitle}>Customize Lobby</h2>
      <p className={styles.modalHint}>
        Choose which widgets to show and drag them to rearrange your layout. Drag them back here, or click the close <b>[×]</b> button, to remove them.
      </p>
      <div className={styles.items}>
        {offGrid.map((family) => {
          const options = sizesOf(family)
          const chosen = sizes[family] ?? options[0]
          const span = widgetSpan(chosen)
          const begin = (e: React.PointerEvent, widget: LobbyWidgetId): void => onStart(widget, e, { width: spanSize(span.x), height: spanSize(span.y) })
          return (
            <div
              key={family}
              className={`${styles.row} ${dragging != null && familyVariants(family).includes(dragging) ? styles.rowDragging : ''}`.trim()}
              onPointerDown={(e) => begin(e, chosen)}
            >
              <button
                type="button"
                className={styles.rowButton}
                aria-label={`Add ${familyTitle(family)}`}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    onPlace(chosen)
                  }
                }}
              />
              <MaskIcon src={dragIcon} size={24} className={styles.grip} />
              <img className={styles.familyIcon} src={FAMILY_ICONS[family]} alt="" width={24} height={24} />
              <span className={styles.rowTitle}>{familyTitle(family)}</span>
              {options.length > 1 && (
                <span className={styles.sizes} role="radiogroup" aria-label={`${familyTitle(family)} size`}>
                  {options.map((w) => (
                    <button
                      key={w}
                      type="button"
                      role="radio"
                      aria-checked={w === chosen}
                      className={`${styles.size} ${w === chosen ? styles.sizeSelected : ''}`.trim()}
                      onPointerDown={(e) => {
                        setSizes((s) => ({ ...s, [family]: w }))
                        e.stopPropagation()
                        const s = widgetSpan(w)
                        onStart(w, e, { width: spanSize(s.x), height: spanSize(s.y) })
                      }}
                      onClick={() => setSizes((s) => ({ ...s, [family]: w }))}
                    >
                      x{widgetCardCount(w)}
                    </button>
                  ))}
                </span>
              )}
            </div>
          )
        })}
      </div>
      <div className={styles.actions}>
        <button type="button" className={styles.button} onClick={onReset}>
          Reset to Default
        </button>
        <span className={styles.spacer} />
        <button type="button" className={`${styles.button} ${styles.fixed}`} onClick={onCancel}>
          CANCEL
        </button>
        <button type="button" className={`${styles.button} ${styles.fixed} ${styles.primary}`} disabled={!changed} onClick={onSave}>
          <img src={checkmark} alt="" width={11} height={11} />
          SAVE
        </button>
      </div>
    </div>
  )
}
