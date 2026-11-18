// Discount attempts whose outcome this tab could not confirm, kept across reloads. Each one is a step the creator
// already paid for or signed: a cancellation that was broadcast but not seen mined, or a coupon that was signed
// but whose POST got no readable answer. Offering the action again before reconciling it would pay for a second
// cancellation or put a second, independently redeemable coupon on sale.
import { type CouponCreation } from '~/lib/coupons'

export type PendingEnd = {
  discountId: string
  txHash: string
  /** The sender's nonce, which is what proves a cancellation with no receipt can never execute. */
  nonce?: number
  chainId: number
  sentAt: number
  /** The receipt says it mined: kept until the server stops reporting the discount running. */
  minedAt?: number
  /** Its nonce went to another transaction (dropped, sped up or replaced): kept until the server's next read. */
  supersededAt?: number
  /** When the discount stops applying anyway; the record is useless past it. */
  expiresAt: number
}
export type UnconfirmedCreate = {
  collectionId: string
  payload: CouponCreation
  /**
   * The creator chose to sign a new one anyway: no longer holding the form back, but kept until it expires as the
   * record that a coupon of theirs may be on sale without this app having seen it stored.
   */
  acknowledged?: boolean
}
type Attempts = { ends: PendingEnd[]; creates: UnconfirmedCreate[] }

const STORAGE_PREFIX = 'collection-discount-attempts:'
// Attempts this tab could not persist (private mode, blocked storage, a full quota). While a key has one it is the
// truth for this tab, whatever storage holds, until a later write persists it; another tab's changes to that key are
// not seen meanwhile. It does not survive a reload: only persisted attempts do.
const unpersisted = new Map<string, Attempts>()

const keyOf = (address: string) => `${STORAGE_PREFIX}${address.toLowerCase()}`
const isNumber = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value)

function isPendingEnd(value: unknown): value is PendingEnd {
  const end = value as Partial<PendingEnd> | null
  return (
    !!end &&
    typeof end.discountId === 'string' &&
    typeof end.txHash === 'string' &&
    isNumber(end.chainId) &&
    isNumber(end.sentAt) &&
    isNumber(end.expiresAt)
  )
}

function isUnconfirmedCreate(value: unknown): value is UnconfirmedCreate {
  const create = value as Partial<UnconfirmedCreate> | null
  const payload = create?.payload as Partial<CouponCreation> | undefined
  return (
    !!create &&
    typeof create.collectionId === 'string' &&
    !!payload &&
    typeof payload.signature === 'string' &&
    typeof payload.signer === 'string' &&
    Array.isArray(payload.collections) &&
    payload.collections.every(collection => typeof collection === 'string') &&
    (create.acknowledged === undefined || typeof create.acknowledged === 'boolean') &&
    isNumber(payload.chainId) &&
    isNumber(payload.discount) &&
    !!payload.checks &&
    isNumber(payload.checks.expiration)
  )
}

function read(address: string): Attempts {
  const key = keyOf(address)
  const local = unpersisted.get(key)
  if (local) return local
  let raw: string | null
  try {
    raw = localStorage.getItem(key)
  } catch {
    return { ends: [], creates: [] }
  }
  if (!raw) return { ends: [], creates: [] }
  try {
    const parsed = JSON.parse(raw) as { ends?: unknown; creates?: unknown }
    // Entries that don't have the shape this build writes are dropped rather than trusted.
    return {
      ends: Array.isArray(parsed.ends) ? parsed.ends.filter(isPendingEnd) : [],
      creates: Array.isArray(parsed.creates) ? parsed.creates.filter(isUnconfirmedCreate) : []
    }
  } catch {
    return { ends: [], creates: [] }
  }
}

function write(address: string, attempts: Attempts): void {
  const key = keyOf(address)
  try {
    if (attempts.ends.length === 0 && attempts.creates.length === 0) localStorage.removeItem(key)
    else localStorage.setItem(key, JSON.stringify(attempts))
    unpersisted.delete(key)
  } catch {
    unpersisted.set(key, attempts)
  }
}

export function pendingEnds(address: string, now = Date.now()): PendingEnd[] {
  const attempts = read(address)
  const live = attempts.ends.filter(end => end.expiresAt > now)
  if (live.length !== attempts.ends.length) write(address, { ...attempts, ends: live })
  return live
}

export function pendingEndOf(address: string, discountId: string): PendingEnd | undefined {
  return pendingEnds(address).find(end => end.discountId === discountId)
}

export function savePendingEnd(address: string, end: PendingEnd): void {
  const attempts = read(address)
  write(address, { ...attempts, ends: [...attempts.ends.filter(e => e.discountId !== end.discountId), end] })
}

export function updatePendingEnd(address: string, discountId: string, patch: Partial<PendingEnd>): void {
  const attempts = read(address)
  write(address, {
    ...attempts,
    ends: attempts.ends.map(end => (end.discountId === discountId ? { ...end, ...patch } : end))
  })
}

export function clearPendingEnd(address: string, discountId: string): void {
  const attempts = read(address)
  write(address, { ...attempts, ends: attempts.ends.filter(end => end.discountId !== discountId) })
}

/** The oldest coupon signed for the collection whose storing is unconfirmed, unless its window already closed. */
export function unconfirmedCreateOf(
  address: string,
  collectionId: string,
  now = Date.now()
): UnconfirmedCreate | undefined {
  const attempts = read(address)
  const live = attempts.creates.filter(create => create.payload.checks.expiration > now)
  if (live.length !== attempts.creates.length) write(address, { ...attempts, creates: live })
  return live.find(create => create.collectionId === collectionId && !create.acknowledged)
}

/** Lets the creator sign a new coupon next to this one, which may still be on sale: kept, but no longer blocking. */
export function acknowledgeUnconfirmedCreate(address: string, signature: string): void {
  const attempts = read(address)
  const wanted = signature.toLowerCase()
  write(address, {
    ...attempts,
    creates: attempts.creates.map(create =>
      create.payload.signature.toLowerCase() === wanted ? { ...create, acknowledged: true } : create
    )
  })
}

/** Adds it next to any other unconfirmed coupon: each one may be on sale, so none may be forgotten. */
export function saveUnconfirmedCreate(address: string, create: UnconfirmedCreate): void {
  const attempts = read(address)
  const signature = create.payload.signature.toLowerCase()
  if (attempts.creates.some(entry => entry.payload.signature.toLowerCase() === signature)) return
  write(address, { ...attempts, creates: [...attempts.creates, create] })
}

/** Forgets one signed coupon, by its signature, once it is known stored or known refused. */
export function clearUnconfirmedCreate(address: string, signature: string): void {
  const attempts = read(address)
  const wanted = signature.toLowerCase()
  write(address, {
    ...attempts,
    creates: attempts.creates.filter(create => create.payload.signature.toLowerCase() !== wanted)
  })
}

/** Forgets everything; for specs. */
export function resetDiscountAttempts(): void {
  unpersisted.clear()
  try {
    for (const key of Object.keys(localStorage)) if (key.startsWith(STORAGE_PREFIX)) localStorage.removeItem(key)
  } catch {
    // Nothing stored.
  }
}
