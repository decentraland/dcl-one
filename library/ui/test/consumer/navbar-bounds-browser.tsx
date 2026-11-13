import { createRoot } from 'react-dom/client'
import { NavBar } from '@dcl/one-ui'
import '../../foundations/foundations.css'
import { nativeNavBarCases } from './navbar-bounds-cases'
const requested = new URLSearchParams(location.search).get('case')
const example = nativeNavBarCases.find((value) => value.id === requested)
if (!example) throw new Error('Unknown native NavBar bounds case')
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
createRoot(document.getElementById('native-bounds-root')!).render(
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
        <a href="/shop/credits" id="balance-slot">
          Credits
        </a>
      ) : undefined
    }
    accountControl={
      example.composition ? (
        <button id="account-slot" type="button">
          Wallet
        </button>
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
