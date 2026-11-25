import { useEffect, useRef, type ComponentType, type ReactNode, type RefObject } from 'react'
import { TabsBar, type TabsBarItem, type TabsBarLinkProps } from '../../components/TabsBar/TabsBar'
import ChromeBody from '../../components/AppChrome/ChromeBody'
import SiteLayout from '../../components/AppChrome/SiteLayout'

export type ShopSection =
  'overview' | 'event' | 'collectibles' | 'names' | 'lands' | 'my-items' | 'activity'
export type ShopNavSet = 'shop' | 'marketplace'
export type ShopNavTab = TabsBarItem & { id: ShopSection }

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
  Link: ComponentType<TabsBarLinkProps>
  search: ReactNode
  children?: ReactNode
}

export function ShopNavigation({
  tabs,
  active,
  iap,
  set,
  label,
  Link,
  search,
  children
}: ShopNavigationProps) {
  const subnavRef = useRef<HTMLDivElement>(null)
  useScrolledMark(subnavRef)
  return (
    <div ref={subnavRef} className="shop-subnav" data-testid="subnav" data-iap={iap || undefined} data-set={set}>
      <div className="shop-subnav__tabs" data-testid="subnav-tabs">
        <TabsBar
          mode="navigation"
          page="shop"
          layout="links"
          label={label}
          items={tabs}
          activeId={active ?? null}
          LinkComponent={Link}
        />
      </div>
      <hr className="shop-subnav__divider" aria-hidden="true" />
      {search}
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
