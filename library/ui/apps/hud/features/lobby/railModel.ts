// The lobby rail's paging, apart from the DOM so it can be tested: a strip of cards that pages by the
// wheel, the arrows or the dots and scrolls freely by dragging, a released drag settling on the nearest
// card. The arrows wrap round past either end; the wheel stops there and each dot goes to its page.

/** Seconds of its release speed a flung drag runs on for, capped at a page. */
export const FLING_PROJECTION = 0.12
/** A pointer that rested longer than this (ms) before its release was not flung. */
export const FLING_STALE_MS = 100
/** Seconds between two wheel pages: the snap's duration. */
export const WHEEL_COOLDOWN = 0.25
/** Pixels a press must travel before it drags the strip instead of clicking a card. */
export const DRAG_THRESHOLD = 8
/** Slack under which the strip counts as having reached its end. */
const END_TOLERANCE = 0.5
/** How far pixel rounding can put the measured end of the strip off a card. */
const PIXEL_ROUNDING = 4

/** The laid-out strip: each card's left edge and right edge in the strip, and the viewport's width. */
export interface RailGeometry {
  lefts: number[]
  rights: number[]
  viewport: number
}

export interface RailState {
  /** The card at the left edge of the viewport since the last snap. */
  card: number
  page: number
  /** How far the strip is slid left, px. */
  offset: number
}

export const RAIL_START: RailState = { card: 0, page: 0, offset: 0 }

export class RailModel {
  constructor(
    readonly geometry: RailGeometry,
    readonly shown: number,
    readonly perPage: number,
    readonly arrowsMoveOneCard: boolean
  ) {}

  get pageCount(): number {
    return Math.ceil(this.shown / this.perPage)
  }

  private cardX(card: number): number {
    return this.geometry.lefts[card] ?? 0
  }

  /** The strip ends at the last shown card; an end within pixel rounding of a card is put on that card. */
  maxOffset(): number {
    const last = Math.min(this.shown, this.geometry.lefts.length) - 1
    if (last < 0) return 0
    const max = Math.max(0, this.geometry.rights[last] - this.geometry.viewport)
    for (let i = 0; i <= last; i++) {
      const x = this.cardX(i)
      if (Math.abs(x - max) <= PIXEL_ROUNDING) return x
    }
    return max
  }

  private atEnd(state: RailState): boolean {
    return state.offset >= this.maxOffset() - END_TOLERANCE
  }

  /** The last cards share the offset of the end. */
  cardOffset(card: number): number {
    return Math.min(this.cardX(card), this.maxOffset())
  }

  private nearest(at: number, count: number, step: number): number {
    const max = this.maxOffset()
    let nearest = 0
    let best = Infinity
    for (let i = 0; i < count; i++) {
      const d = Math.abs(at - Math.min(this.cardX(i * step), max))
      if (d >= best) continue
      best = d
      nearest = i
    }
    return nearest
  }

  private cardAt(at: number): number {
    return this.nearest(at, this.shown, 1)
  }

  private pageAt(at: number): number {
    return this.nearest(at, this.pageCount, this.perPage)
  }

  private toCard(card: number, page: number): RailState {
    return { card, page, offset: this.cardOffset(card) }
  }

  /** The first card of `page`, clamped to the filled pages. */
  snapTo(page: number): RailState {
    const p = Math.min(Math.max(page, 0), Math.max(0, this.pageCount - 1))
    return this.toCard(p * this.perPage, p)
  }

  private pageAfter(state: RailState): number {
    return Math.floor(state.card / this.perPage) + 1
  }

  private pageBefore(state: RailState): number {
    return Math.floor(Math.max(0, state.card - 1) / this.perPage)
  }

  private stepToCard(state: RailState, target: number): RailState {
    let card = target
    if (card < 0) card = this.shown - 1
    else if (card >= this.shown || (card > state.card && this.atEnd(state))) card = 0
    card = this.cardAt(this.cardOffset(card))
    return this.toCard(card, this.pageAt(this.cardOffset(card)))
  }

  /** From the start the rail wraps round to the end. */
  previous(state: RailState): RailState {
    if (this.arrowsMoveOneCard) return this.stepToCard(state, state.card - 1)
    return this.snapTo(state.card === 0 ? this.pageCount - 1 : this.pageBefore(state))
  }

  /** Past the last page the rail wraps round to the first, once the strip has reached its end. */
  next(state: RailState): RailState {
    if (this.arrowsMoveOneCard) return this.stepToCard(state, state.card + 1)
    const page = this.pageAfter(state)
    return this.snapTo(page < this.pageCount || !this.atEnd(state) ? page : 0)
  }

  /** A wheel turn pages forward (down) or back (up), stopping at the ends. */
  wheel(state: RailState, down: boolean): RailState {
    return this.snapTo(down ? this.pageAfter(state) : this.pageBefore(state))
  }

  /** Where a drag at `offset` lands, flung on by `velocity` px/s (positive: the pointer moved right). */
  dragEnd(offset: number, velocity: number): RailState {
    const stride = this.geometry.lefts.length > 1 ? this.geometry.lefts[1] - this.geometry.lefts[0] : 0
    const page = this.perPage * stride
    const fling = Math.min(Math.max(-velocity * FLING_PROJECTION, -page), page)
    const card = this.cardAt(offset + fling)
    return this.toCard(card, this.pageAt(this.cardOffset(card)))
  }

  /** The offset a drag from `start` reaches after the pointer moved `dx`, held inside the strip. */
  dragOffset(start: number, dx: number): number {
    return Math.min(Math.max(start - dx, 0), this.maxOffset())
  }

  /** Re-placed on the card it was on, once the cards are laid out (or after a change of cards). */
  settle(state: RailState): RailState {
    return { ...state, offset: this.cardOffset(state.card) }
  }
}
