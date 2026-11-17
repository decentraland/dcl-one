import { createRoot } from 'react-dom/client'
import { Button, NavBar, NavCreditsBalance } from '@dcl/one-ui'
import '../../foundations/foundations.css'
import { nativeNavBarCases } from './navbar-bounds-cases'
const requested = new URLSearchParams(location.search).get('case')
const example = nativeNavBarCases.find((value) => value.id === requested)
const root = createRoot(document.getElementById('native-bounds-root')!)
// Opened by hand without a known case: list the cases so the fixture can be browsed.
if (!example) {
  root.render(
    <ul>
      {nativeNavBarCases.map(({ id }) => (
        <li key={id}>
          <a href={`?case=${id}`} style={{ color: 'inherit' }}>
            {id}
          </a>
        </li>
      ))}
    </ul>
  )
  throw new Error('Unknown native NavBar bounds case')
}
const authentication = example.signedIn
  ? {
      account: { address: '0x0000000000000000000000000000000000000001', name: 'Ada' },
      notifications: { count: 12, onSelect: () => {} },
      accountActions: [
        { label: 'Profile', href: '/profile' },
        { label: 'Sign out', onSelect: () => {} }
      ]
    }
  : { signIn: { href: '/sign-in' } }
root.render(
  <NavBar
    {...authentication}
    sections={[
      {
        label: 'Discover',
        items: [
          { label: 'Places', href: '/places' },
          { label: 'Events', href: '/events' }
        ]
      },
      { label: 'Shop', href: '/shop' },
      { label: 'Create', href: '/create' },
      {
        label: 'Learn',
        items: [
          { label: 'Documentation', href: '/docs' },
          { label: 'Blog', href: '/blog' }
        ]
      }
    ]}
    balances={example.balances}
    surface={example.surface}
    balanceAction={
      example.composition ? (
        <NavCreditsBalance
          id="balance-slot"
          href="/shop/credits"
          label="30"
          aria-label="30 credits"
        />
      ) : undefined
    }
    accountControl={
      example.composition ? (
        <Button id="account-slot" variant="secondary" size="sm">
          Wallet
        </Button>
      ) : undefined
    }
    mobileSecondaryNavigation={
      example.composition
        ? ({ closeDrawer }) => (
            <nav aria-label="Profile tabs">
              <a href="/profile/photos" onClick={closeDrawer}>
                Photos
              </a>
              <a href="/profile/places" onClick={closeDrawer}>
                Places
              </a>
            </nav>
          )
        : undefined
    }
  />
)
