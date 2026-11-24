import {
  useEffect,
  useRef,
  type AnchorHTMLAttributes,
  type ComponentType,
  type ReactNode,
  type RefObject
} from 'react'
import ChromeBody from '../../components/AppChrome/ChromeBody'
import SiteLayout from '../../components/AppChrome/SiteLayout'

export type ShopSection =
  'overview' | 'event' | 'collectibles' | 'names' | 'lands' | 'my-items' | 'activity'
export type ShopNavSet = 'shop' | 'marketplace'
export type ShopNavTab = { id: ShopSection; label: string; href: string; mobileOnly?: boolean }

type ShopChromeProps = {
  topbar?: ReactNode
  tools: ReactNode
  footer?: ReactNode
  children?: ReactNode
}

export default function ShopChrome({ topbar, tools, footer, children }: ShopChromeProps) {
  return (
    <SiteLayout className="ui2" navigation={topbar}>
      <ChromeBody className="shop-chrome" ariaLabel="Shop" tools={tools} footer={footer}>
        {children}
      </ChromeBody>
    </SiteLayout>
  )
}

type ShopNavigationProps = {
  tabs: ShopNavTab[]
  active?: ShopSection
  iap: boolean
  set: ShopNavSet
  label: string
  Link: ComponentType<Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'href'> & { to: string }>
  children?: ReactNode
}

export function ShopNavigation({
  tabs,
  active,
  iap,
  set,
  label,
  Link,
  children
}: ShopNavigationProps) {
  const tabsRef = useRef<HTMLElement>(null)
  const subnavRef = useRef<HTMLDivElement>(null)
  useScrolledMark(subnavRef)
  useEffect(() => {
    const nav = tabsRef.current
    const current = nav?.querySelector<HTMLElement>('[aria-current="page"]')
    if (!nav || !current || nav.scrollWidth <= nav.clientWidth) return
    const left = current.offsetLeft - nav.offsetLeft
    if (left < nav.scrollLeft || left + current.offsetWidth > nav.scrollLeft + nav.clientWidth) {
      nav.scrollLeft = Math.max(0, left - (nav.clientWidth - current.offsetWidth) / 2)
    }
  }, [active, tabs])
  return (
    <div ref={subnavRef} className="shop-subnav" data-testid="subnav" data-iap={iap || undefined} data-set={set}>
      <nav ref={tabsRef} className="shop-subnav__tabs" aria-label={label} data-testid="subnav-tabs">
        {tabs.map((tab) => (
          <Link
            key={tab.id}
            to={tab.href}
            className={tab.id === active ? 'active' : undefined}
            aria-current={tab.id === active ? 'page' : undefined}
            data-event={tab.id === 'event' || undefined}
            data-mobile-only={tab.mobileOnly || undefined}
          >
            {tab.label}
          </Link>
        ))}
      </nav>
      <hr className="shop-subnav__divider" aria-hidden="true" />
      {children}
    </div>
  )
}

const SCROLL_ENTER = 24
const SCROLL_EXIT = 4

// The bars deepen once the page scrolls, with hysteresis so a page resting near the threshold does not flicker.
// The scroll event sets the marks itself, so the bars change in that frame, not after a render. The body mark is
// for the site nav's rule (shoptheme.css).
function useScrolledMark(subnav: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    let on = false
    const apply = (next: boolean) => {
      on = next
      subnav.current?.toggleAttribute('data-scrolled', next)
      document.body.toggleAttribute('data-shop-scrolled', next)
    }
    const onScroll = () => {
      const next = on ? window.scrollY > SCROLL_EXIT : window.scrollY > SCROLL_ENTER
      if (next !== on) apply(next)
    }
    apply(window.scrollY > SCROLL_ENTER)
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      window.removeEventListener('scroll', onScroll)
      document.body.removeAttribute('data-shop-scrolled')
    }
  }, [subnav])
}
