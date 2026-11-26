import { useState, type MouseEvent, type ReactNode } from 'react'
import {
  Button,
  GeneralAudio,
  GeneralControls,
  GeneralFilters,
  GeneralGraphics,
  NavBar,
  NavCreditsBalance,
  NavMenuItem,
  NavMenuTab,
  NavNotificationsButton,
  NavProfileButton,
  SearchBar,
  ShopCart,
  Tabs,
  TabsBar,
  TitleBar,
  decentralandPublicNavSections,
  type TitleBarProps,
  type TitleBarSection,
  Pager,
  BrowseControl,
  BrowseLoading,
  BrowsePanel,
  BrowseToolbar,
  MenuDivider,
  MenuList,
  MenuPanel,
  MenuRow,
  AnchoredMenu,
  ContextMenu,
  KebabMenu,
  Submenu
} from '@dcl/one-ui'
import type { GalleryEntry } from '../registry'
import { covers, people, placeholderFace } from './data'
import layout from './layout.module.css'

function AnchoredMenuExample() {
  const [opener, setOpener] = useState<HTMLSpanElement | null>(null)
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null)
  return (
    <div style={{ minHeight: 160 }}>
      <span ref={setOpener} style={{ display: 'inline-block' }}>
        <Button
          onClick={(event: MouseEvent<HTMLElement>) =>
            setAnchor({ x: event.clientX, y: event.clientY })
          }
        >
          Photo options
        </Button>
      </span>
      <AnchoredMenu
        open={anchor != null}
        anchor={anchor ?? { x: 0, y: 0 }}
        aria-label="Photo options"
        ignore={opener}
        onClose={() => setAnchor(null)}
        items={[{ label: 'Share' }, { label: 'Download' }, { label: 'Delete', danger: true }]}
      />
    </div>
  )
}

const publicSections = decentralandPublicNavSections('explore', '/explore', {
  marketplaceEligible: true
})

const navBarBase = {
  sections: publicSections.desktop,
  mobileSections: publicSections.mobile
}

const titleSections: TitleBarSection[] = [
  { id: 'graphics', label: 'Graphics', icon: <GeneralGraphics size={24} aria-hidden="true" /> },
  { id: 'audio', label: 'Audio', icon: <GeneralAudio size={24} aria-hidden="true" /> },
  { id: 'controls', label: 'Controls', icon: <GeneralControls size={24} aria-hidden="true" /> }
]

type TitleBarControls = Pick<TitleBarProps, 'variant' | 'controls'>

function Search() {
  const [value, setValue] = useState('')
  return <SearchBar label="Search" placeholder="Search" value={value} onValueChange={setValue} />
}

const filters = (
  <Button
    size="sm"
    variant="secondary"
    leadingIcon={<GeneralFilters size={20} aria-hidden="true" />}
  >
    Filters
  </Button>
)

const titleBarVariants: TitleBarControls[] = [
  { variant: 'settings' },
  { variant: 'backpack', controls: { filters, search: <Search /> } },
  {
    variant: 'events',
    controls: {
      date: (
        <Button size="sm" variant="secondary">
          Today
        </Button>
      ),
      create: (
        <Button size="sm" variant="primary">
          Create event
        </Button>
      )
    }
  },
  { variant: 'places', controls: { filters, search: <Search /> } },
  {
    variant: 'shop',
    controls: {
      filters,
      sort: (
        <Button size="sm" variant="secondary">
          Newest
        </Button>
      ),
      search: <Search />,
      cart: (
        <Button size="sm" variant="iconOnly" aria-label="Cart">
          <ShopCart size={20} aria-hidden="true" />
        </Button>
      )
    }
  }
]

function TitleBarDemo({ controls }: { controls: TitleBarControls }) {
  const [activeId, setActiveId] = useState('graphics')
  return (
    <TitleBar
      {...(controls as TitleBarProps)}
      sections={titleSections}
      activeId={activeId}
      onSelect={setActiveId}
    />
  )
}

function PanelTabsDemo({ variant }: { variant: 'underline' | 'compact' | 'pill' }) {
  const [selected, setSelected] = useState('all')
  return (
    <TabsBar label={`${variant} tabs`} variant={variant} scrollable>
      {['all', 'wearables', 'emotes', 'names', 'land'].map((id) => (
        <Tabs
          key={id}
          label={id[0]!.toUpperCase() + id.slice(1)}
          variant={variant}
          selected={selected === id}
          onClick={() => setSelected(id)}
        />
      ))}
    </TabsBar>
  )
}

// Selecting runs the tabs' selection motion; a demo link selects in place instead of navigating the canvas.
function stay(event: MouseEvent) {
  if ((event.target as Element).closest('a')) event.preventDefault()
}

function TabsVariantDemo({ variant }: { variant: 'underline' | 'compact' | 'pill' }) {
  const [selected, setSelected] = useState('wearables')
  return (
    <TabsBar label={`${variant} tabs`} variant={variant}>
      {['wearables', 'emotes'].map((id) => (
        <Tabs
          key={id}
          label={id[0]!.toUpperCase() + id.slice(1)}
          variant={variant}
          selected={selected === id}
          onClick={() => setSelected(id)}
        />
      ))}
    </TabsBar>
  )
}

function TabsDeviceDemo({ device }: { device: 'mobile' | 'desktop' }) {
  const [selected, setSelected] = useState('button')
  return (
    <TabsBar label={`${device} tabs`} onClick={stay} scrollable>
      <Tabs
        label={`${device} button`}
        device={device}
        selected={selected === 'button'}
        onClick={() => setSelected('button')}
      />
      <Tabs
        label={`${device} link`}
        device={device}
        href="#tab"
        selected={selected === 'link'}
        onClick={() => setSelected('link')}
      />
    </TabsBar>
  )
}

function TabsRouteDemo() {
  const [current, setCurrent] = useState('overview')
  return (
    <nav aria-label="Route tabs" className={layout.row} onClick={stay}>
      {[
        { id: 'overview', label: 'Overview' },
        { id: 'collections', label: 'Collections' }
      ].map(({ id, label }) => (
        <Tabs
          key={id}
          mode="navigation"
          label={label}
          href={`#${id}`}
          selected={current === id}
          onClick={() => setCurrent(id)}
        />
      ))}
    </nav>
  )
}

function RouteTabsBarDemo({
  initial,
  shop
}: {
  initial: string
  shop?: { search: ReactNode; actions: ReactNode }
}) {
  const [activeId, setActiveId] = useState(initial)
  const common = { items: routeItems, activeId, onSelect: setActiveId }
  return (
    <div onClick={stay}>
      {shop ? (
        <TabsBar mode="navigation" page="shop" {...common} {...shop} />
      ) : (
        <TabsBar mode="navigation" {...common} />
      )}
    </div>
  )
}

const routeItems = [
  { id: 'overview', label: 'Overview', href: '#overview' },
  { id: 'collections', label: 'Collections', href: '#collections' },
  { id: 'curation', label: 'Curation', href: '#curation', hiddenOnMobile: true }
]

export const navigationEntries: GalleryEntry[] = [
  {
    name: 'NavBar',
    group: 'Navigation',
    summary:
      'The site header. It switches to the drawer layout below the 1024px desktop breakpoint, so compare widths.',
    components: ['NavBar'],
    minHeight: 480,
    examples: [
      {
        id: 'signed-out',
        title: 'Signed out',
        render: () => <NavBar {...navBarBase} signIn={{ href: '#sign-in' }} />
      },
      {
        id: 'signed-in',
        title: 'Signed in, with balances and unread notifications',
        render: () => (
          <NavBar
            {...navBarBase}
            account={{ address: '0x0000000000000000000000000000000000000001', name: 'Ada' }}
            balances={{ credits: 8, mana: 2.5 }}
            notifications={{ count: 3, onSelect: () => {} }}
            accountActions={[
              { label: 'Profile', href: '#profile' },
              { label: 'Sign out', onSelect: () => {}, separated: true }
            ]}
          />
        )
      },
      {
        id: 'overlay',
        title: 'Overlay surface over a cover',
        render: () => (
          <div style={{ backgroundImage: `url("${covers.plaza}")`, backgroundSize: 'cover' }}>
            <NavBar {...navBarBase} surface="overlay" signIn={{ href: '#sign-in' }} />
          </div>
        )
      }
    ]
  },
  {
    name: 'NavMenuTab',
    group: 'Navigation',
    summary: 'A top-level header tab, optionally opening a menu.',
    components: ['NavMenuTab'],
    examples: [
      {
        id: 'states',
        title: 'Default, current page and menu',
        render: () => (
          <div className={layout.row}>
            <NavMenuTab label="Discover" href="#discover" />
            <NavMenuTab label="Shop" href="#shop" active />
            <NavMenuTab label="Learn" as="button" type="button" hasMenu />
            <NavMenuTab label="Create" as="button" type="button" hasMenu expanded />
          </div>
        )
      }
    ]
  },
  {
    name: 'NavMenuItem',
    group: 'Navigation',
    summary: 'An entry in a header menu.',
    components: ['NavMenuItem'],
    examples: [
      {
        id: 'items',
        title: 'Menu entries',
        render: () => (
          <div className={layout.stack}>
            <NavMenuItem label="Docs" href="#docs" />
            <NavMenuItem label="Blog" href="#blog" />
            <NavMenuItem label="FAQ" as="button" type="button" />
          </div>
        )
      }
    ]
  },
  {
    name: 'NavCreditsBalance',
    group: 'Navigation',
    summary: 'The Credits balance shown in the header.',
    components: ['NavCreditsBalance'],
    examples: [
      {
        id: 'balances',
        title: 'Link and plain balance',
        render: () => (
          <div className={layout.row}>
            <NavCreditsBalance label="7.78" href="#credits" />
            <NavCreditsBalance label="0" href="#credits" />
            <NavCreditsBalance label="1,250" as="span" />
          </div>
        )
      }
    ]
  },
  {
    name: 'NavNotificationsButton',
    group: 'Navigation',
    summary: 'The bell that opens the notifications panel.',
    components: ['NavNotificationsButton'],
    examples: [
      {
        id: 'states',
        title: 'Closed and open',
        render: () => (
          <div className={layout.row}>
            <NavNotificationsButton state="closed" />
            <NavNotificationsButton state="open" label="Notifications, 3 unread" />
          </div>
        )
      }
    ]
  },
  {
    name: 'NavProfileButton',
    group: 'Navigation',
    summary: 'The account disc in the header.',
    components: ['NavProfileButton'],
    examples: [
      {
        id: 'faces',
        title: 'Face picture, fallback and name colour',
        render: () => (
          <div className={layout.row}>
            <NavProfileButton imageUrl={people.ada.picture} fallbackSrc={placeholderFace} />
            <NavProfileButton fallbackSrc={placeholderFace} color={people.grace.color} />
          </div>
        )
      }
    ]
  },
  {
    name: 'Tabs',
    group: 'Navigation',
    summary: 'A single tab: panel tabs inside TabsBar, or a navigation link tab.',
    components: ['Tabs'],
    examples: [
      {
        id: 'variants',
        title: 'Underline, compact and pill',
        note: 'Click or use the arrow keys to select a tab.',
        render: () => (
          <div className={layout.stack}>
            {(['underline', 'compact', 'pill'] as const).map((variant) => (
              <TabsVariantDemo key={variant} variant={variant} />
            ))}
          </div>
        )
      },
      {
        id: 'devices',
        title: 'Fixed device sizes',
        render: () => (
          <div className={layout.rows}>
            {(['mobile', 'desktop'] as const).map((device) => (
              <TabsDeviceDemo key={device} device={device} />
            ))}
          </div>
        )
      },
      {
        id: 'navigation',
        title: 'Navigation mode',
        note: 'Clicking a route moves the current page here without navigating.',
        render: () => <TabsRouteDemo />
      }
    ]
  },
  {
    name: 'TabsBar',
    group: 'Navigation',
    summary: 'A tab list (panel mode) or route tabs with caller controls (navigation mode).',
    components: ['TabsBar'],
    examples: [
      {
        id: 'hud',
        title: 'Explorer HUD strips: pill, underline, section, shop, subtab and a filled underline',
        render: () => (
          <div className={layout.stack} style={{ minWidth: 0, maxWidth: '100%' }}>
            {(['pill', 'underline', 'section', 'shop', 'subtab'] as const).map((variant) => (
              <TabsBar
                key={variant}
                appearance="hud"
                variant={variant}
                label={`${variant} strip`}
                scrollable
                style={{ maxWidth: '100%' }}
              >
                <Tabs appearance="hud" label="Wearables" selected />
                <Tabs appearance="hud" label="Emotes" />
                <Tabs appearance="hud" label="Unavailable" disabled />
              </TabsBar>
            ))}
            <div style={{ height: 44 }}>
              <TabsBar appearance="hud" variant="underline" fill label="filled strip">
                <Tabs appearance="hud" label="Friends" selected />
                <Tabs appearance="hud" label="Requests" />
              </TabsBar>
            </div>
          </div>
        )
      },
      {
        id: 'underline',
        title: 'Panel, underline',
        render: () => <PanelTabsDemo variant="underline" />
      },
      { id: 'compact', title: 'Panel, compact', render: () => <PanelTabsDemo variant="compact" /> },
      { id: 'pill', title: 'Panel, pill', render: () => <PanelTabsDemo variant="pill" /> },
      {
        id: 'navigation-create',
        title: 'Navigation, create page',
        note: 'The last route is hidden on narrow layouts.',
        render: () => <RouteTabsBarDemo initial="overview" />
      },
      {
        id: 'navigation-shop',
        title: 'Navigation, shop page with search and actions',
        render: () => (
          <RouteTabsBarDemo
            initial="collections"
            shop={{
              search: <Search />,
              actions: (
                <Button size="sm" variant="iconOnly" aria-label="Cart">
                  <ShopCart size={20} aria-hidden="true" />
                </Button>
              )
            }}
          />
        )
      }
    ]
  },
  {
    name: 'TitleBar',
    group: 'Navigation',
    summary: 'The title row of an Explorer panel, with its sections and controls per variant.',
    components: ['TitleBar'],
    examples: titleBarVariants.map((controls) => ({
      id: controls.variant,
      title: controls.variant[0]!.toUpperCase() + controls.variant.slice(1),
      render: () => <TitleBarDemo controls={controls} />
    }))
  },
  {
    name: 'BrowsePage',
    group: 'Navigation',
    summary: 'The frame of a browse page: a toolbar of tabs and controls over a results panel.',
    components: ['BrowseToolbar', 'BrowseControl', 'BrowsePanel', 'BrowseLoading'],
    examples: [
      {
        id: 'frame',
        title: 'A toolbar over a loading panel and a panel with results',
        render: () => (
          <div className={layout.strip}>
            <div style={{ width: 720 }}>
              <BrowseToolbar tabs={<strong>Wearables</strong>}>
                <BrowseControl size="search">
                  <input aria-label="Search the shop" placeholder="Search" />
                </BrowseControl>
                <BrowseControl size="select">
                  <select aria-label="Sort by">
                    <option>Newest</option>
                  </select>
                </BrowseControl>
              </BrowseToolbar>
              <div className={layout.stack}>
                <BrowsePanel label="Results, loading" busy>
                  <BrowseLoading />
                </BrowsePanel>
                <BrowsePanel label="Results">Nothing on sale here.</BrowsePanel>
              </div>
            </div>
          </div>
        )
      }
    ]
  },
  {
    name: 'ContextMenu',
    group: 'Navigation',
    summary: 'A stacked menu of rows, with the forms that open it from a kebab button or a point.',
    components: ['ContextMenu', 'Submenu', 'KebabMenu', 'AnchoredMenu'],
    examples: [
      {
        id: 'menu',
        title: 'A header, items, a switch, exclusive options and a danger row',
        render: () => (
          <ContextMenu
            aria-label="Player options"
            items={[
              { kind: 'header', name: 'Aria', tag: '#a3f1', address: '0x1234567890abcdef1234' },
              { kind: 'separator' },
              { label: 'View profile' },
              { kind: 'toggle', label: 'Mute', checked: true },
              { kind: 'title', label: 'Sort by' },
              { kind: 'option', label: 'Newest', checked: true },
              { kind: 'option', label: 'Oldest', checked: false },
              { kind: 'separator' },
              { label: 'Block', danger: true }
            ]}
          />
        )
      },
      {
        id: 'sheet',
        title: 'The flat sheet, with taller rows',
        render: () => (
          <ContextMenu
            variant="sheet"
            aria-label="Conversation options"
            items={[
              { label: 'View profile' },
              { kind: 'toggle', label: 'Notifications', checked: false },
              { kind: 'separator' },
              { label: 'Delete chat history', danger: true }
            ]}
          />
        )
      },
      {
        id: 'submenu',
        title: 'A row that opens a list beside it, on hover or press',
        render: () => (
          <div style={{ width: 240, minHeight: 150 }}>
            <Submenu label="Invite to Community">
              <ContextMenu items={[{ label: 'Pixel Builders' }, { label: 'Neon Club' }]} scroll />
            </Submenu>
          </div>
        )
      },
      {
        id: 'kebab',
        title: 'Opened from a kebab button, in both sizes',
        render: () => (
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 12, minHeight: 120 }}>
            <KebabMenu
              label="Conversation options"
              items={(close) => [{ label: 'View Community', onClick: close }]}
            />
            <KebabMenu
              size="sm"
              label="Participant options"
              items={(close) => [{ label: 'View profile', onClick: close }]}
            />
          </div>
        )
      },
      {
        id: 'anchored',
        title: 'Opened from the point of a press: press the button',
        render: () => <AnchoredMenuExample />
      }
    ]
  },
  {
    name: 'MenuPanel',
    group: 'Navigation',
    summary: 'The panel a navigation button opens, with its rows.',
    components: ['MenuPanel', 'MenuList', 'MenuRow', 'MenuDivider'],
    examples: [
      {
        id: 'panel',
        title: 'A titled panel with two groups of rows',
        render: () => (
          <MenuPanel label="Discover" title="DISCOVER">
            <MenuList>
              <MenuRow label="Events" hint="X" trailing={<strong>3</strong>} />
              <MenuRow label="Places" hint="C" />
            </MenuList>
            <MenuDivider />
            <MenuList>
              <MenuRow label="Communities" />
              <MenuRow label="Unavailable" disabled />
            </MenuList>
          </MenuPanel>
        )
      }
    ]
  },
  {
    name: 'Pager',
    group: 'Navigation',
    summary: 'Previous and next arrows around a window of page numbers.',
    components: ['Pager'],
    examples: [
      {
        id: 'variants',
        title: 'The two variants, on the third of nine pages',
        render: () => <PagerExample />
      }
    ]
  }
]

function PagerExample() {
  const [page, setPage] = useState(2)
  return (
    <div className={layout.rows}>
      <div className={layout.strip}>
        <Pager page={page} count={9} onChange={setPage} />
      </div>
      <div className={layout.strip}>
        <Pager variant="shop" page={page} count={9} onChange={setPage} />
      </div>
    </div>
  )
}
