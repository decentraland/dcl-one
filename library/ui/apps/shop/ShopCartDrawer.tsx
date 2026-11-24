import {
  useLayoutEffect,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
  type RefObject,
  type ComponentType
} from 'react'
import { createPortal } from 'react-dom'
import { GeneralClose, GeneralDelete } from '../../foundations/icons/general'
import styles from './ShopCartDrawer.module.css'

export type ShopCartDrawerLinkProps = {
  to: string
  children: ReactNode
  className: string | undefined
  'data-variant': string
  prefetch?: 'render' | 'none'
  onClick?: () => void
}

export type ShopCartDrawerLine = {
  id: string
  name: string
  thumbnail?: string | null
  background: string
  stroke: string
  flap: ReactNode
  creator?: (className: string | undefined) => ReactNode
  price: number | null
  quantity: number
  stepper?: { atStockCap: boolean; increase: () => void; decrease: () => void }
  remove: () => void
}

type ShopCartDrawerProps = {
  open: boolean
  justAdded: number
  getContents: () => { count: number; lines: ShopCartDrawerLine[]; total: number | null }
  credits: (
    amount: number,
    className: string | undefined,
    coinClassName: string | undefined,
    testId?: string
  ) => ReactNode
  cartHref: string
  itemsHref: string
  onCartPage: boolean
  Link: ComponentType<ShopCartDrawerLinkProps>
  emptyArt: string
  close: () => void
}
const Minus = () => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden>
    <path d="M3.5 8h9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
)

const Plus = () => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden>
    <path d="M8 3.5v9M3.5 8h9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
)

const CheckCircle = () => (
  <svg viewBox="0 0 20 20" fill="none" aria-hidden focusable="false">
    <circle cx="10" cy="10" r="10" fill="#1ea672" />
    <path
      d="M5.8 10.3l2.7 2.7 5.7-6"
      stroke="var(--dcl-neutrals-white)"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
)

// A single cart line: thumbnail, name, creator, quantity stepper, price, delete. A primary mint holds
// several copies (minus floored at 1, plus up to the remaining stock) and shows the line subtotal; an
// existing token is unique, so it has no stepper.
function DrawerLine({
  line,
  credits
}: {
  line: ShopCartDrawerLine
  credits: ShopCartDrawerProps['credits']
}) {
  const { quantity, stepper } = line
  return (
    <li className={styles.card} data-testid="cart-drawer-line">
      <div
        className={styles.thumb}
        data-thumb
        style={{ '--tile-bg': line.background, '--tile-stroke': line.stroke } as CSSProperties}
      >
        {line.thumbnail ? <img src={line.thumbnail} alt={line.name} /> : null}
        {line.flap}
      </div>
      <div className={styles.info}>
        <div data-desc>
          <div className={styles.name} title={line.name}>
            {line.name}
          </div>
          {line.creator?.(styles.by)}
        </div>
        <div className={styles.bottom}>
          {stepper ? (
            <div className={styles.stepper}>
              <button
                type="button"
                className={styles.step}
                onClick={stepper.decrease}
                disabled={quantity <= 1}
                aria-label={`Decrease quantity of ${line.name}`}
              >
                <Minus />
              </button>
              <span className={styles.qty}>{quantity}</span>
              <button
                type="button"
                className={styles.step}
                onClick={stepper.increase}
                disabled={stepper.atStockCap}
                aria-label={`Increase quantity of ${line.name}`}
              >
                <Plus />
              </button>
            </div>
          ) : null}
          {line.price !== null ? credits(line.price, styles.price, styles.coin) : null}
        </div>
      </div>
      <button
        type="button"
        className={styles.del}
        onClick={line.remove}
        aria-label={`Remove ${line.name}`}
        title="Remove"
      >
        <GeneralDelete aria-hidden="true" size={24} />
      </button>
    </li>
  )
}

// A browser without modal dialogs still shows and hides the drawer; only the inert page is lost.
function show(dialog: HTMLDialogElement) {
  if (dialog.open) return
  if (typeof dialog.showModal === 'function') dialog.showModal()
  else dialog.setAttribute('open', '')
}

function hide(dialog: HTMLDialogElement) {
  if (!dialog.open) return
  if (typeof dialog.close === 'function') dialog.close()
  else dialog.removeAttribute('open')
}

// Go to cart preloads the cart page's code once the panel has slid in (a cancelled slide counts), so the slide keeps
// the main thread and the network. Root's data does not reload between Shop pages either (shouldRevalidate in
// apps/sites/src/routes/root.tsx), so the click waits for no server request. The in-world HUD's router has no
// preload, so there it stays a plain link. Go to cart only navigates: the drawer closes when the page changes, so
// the click does not wait for the dialog's close either. On the cart page the link changes no page, so there the
// click closes the drawer.
function DrawerCtas({
  panel,
  onClose,
  Link,
  cartHref,
  onCartPage
}: { panel: RefObject<HTMLElement | null>; onClose: () => void } & Pick<
  ShopCartDrawerProps,
  'Link' | 'cartHref' | 'onCartPage'
>) {
  const [slid, setSlid] = useState(false)
  useEffect(() => {
    const settle = () => setSlid(true)
    void Promise.all((panel.current?.getAnimations?.() ?? []).map((a) => a.finished)).then(
      settle,
      settle
    )
  }, [panel])
  return (
    <div className={styles.ctas}>
      <button type="button" className={styles.cta} data-variant="glass" onClick={onClose}>
        Continue shopping
      </button>
      <Link
        to={cartHref}
        prefetch={slid ? 'render' : 'none'}
        className={styles.cta}
        data-variant="primary"
        onClick={onCartPage ? onClose : undefined}
      >
        Go to cart
      </Link>
    </div>
  )
}

export function ShopCartDrawer({
  open,
  justAdded,
  getContents,
  credits,
  itemsHref,
  cartHref,
  onCartPage,
  Link,
  emptyArt,
  close
}: ShopCartDrawerProps) {
  const dialogRef = useRef<HTMLDialogElement>(null)
  const panelRef = useRef<HTMLElement>(null)

  // The dialog leaves the document only after its own close() has given focus back.
  const [mounted, setMounted] = useState(false)
  if (open && !mounted) setMounted(true)

  // A modal <dialog>: the browser moves focus into it, keeps Tab inside, makes the page inert, closes it on Escape,
  // and on close gives focus back where it was without a scroll of the page. No auto-dismiss: it stays until
  // dismissed. A layout effect, so the panel's slide has started before DrawerCtas looks for it.
  useLayoutEffect(() => {
    const dialog = dialogRef.current
    if (!dialog) return
    if (open) {
      show(dialog)
    } else {
      hide(dialog)
      setMounted(false)
    }
  }, [open, mounted])

  if (!mounted || typeof document === 'undefined') return null
  const { count, lines, total } = getContents()
  const isEmpty = lines.length === 0
  // Portalled to <body>, outside the page's stacking contexts. The close event follows a close the browser makes
  // itself; a dialog already shown again by then stays open.
  return createPortal(
    <dialog
      ref={dialogRef}
      className={styles.root}
      aria-label="Shopping cart"
      data-testid="cart-drawer"
      onCancel={close}
      onClose={() => {
        if (!dialogRef.current?.open) close()
      }}
    >
      <div className={styles.scrim} onClick={close} />
      <aside className={styles.panel} ref={panelRef}>
        <header className={styles.head}>
          <h2 className={styles.title}>{`Shopping Cart (${count})`}</h2>
          <button type="button" className={styles.close} onClick={close} aria-label="Close">
            <GeneralClose aria-hidden="true" size={18} />
          </button>
        </header>

        <div className={styles.body}>
          {isEmpty ? (
            <div className={styles.empty} data-testid="cart-drawer-empty">
              <img className={styles.emptyArt} src={emptyArt} alt="" aria-hidden />
              <p className={styles.emptyTitle}>Nothing here... yet!</p>
              <p className={styles.emptyBody}>
                Explore the Shop and discover your next favorite item.
              </p>
              <Link to={itemsHref} className={styles.cta} data-variant="glass" onClick={close}>
                Start shopping
              </Link>
            </div>
          ) : null}

          {justAdded > 0 ? (
            <div className={styles.banner} data-testid="cart-drawer-banner">
              <span className={styles.bannerCheck}>
                <CheckCircle />
              </span>
              <p>
                <strong>{justAdded === 1 ? '1 Item' : `${justAdded} Items`}</strong> successfully
                added to cart!
              </p>
            </div>
          ) : null}

          <ul className={styles.list}>
            {lines.map((line) => (
              <DrawerLine key={line.id} line={line} credits={credits} />
            ))}
          </ul>
        </div>

        {/* No total and nothing to check out on an empty cart: the empty state carries its own CTA. */}
        {isEmpty ? null : (
          <footer className={styles.foot}>
            <div className={styles.total}>
              <span className={styles.totalLabel}>
                Total: {count === 1 ? '1 item' : `${count} items`}
              </span>
              {total !== null
                ? credits(total, styles.totalVal, styles.coin, 'cart-drawer-total')
                : null}
            </div>
            {/* Dismiss on the left, advance on the right, and advancing stops at the cart. */}
            <DrawerCtas
              Link={Link}
              cartHref={cartHref}
              onCartPage={onCartPage}
              panel={panelRef}
              onClose={close}
            />
          </footer>
        )}
      </aside>
    </dialog>,
    document.body
  )
}
