import { createRoot } from 'react-dom/client'
import {
  Button,
  NavBar,
  NavCreditsBalance,
  NavMenuItem,
  decentralandNavSections,
  decentralandPublicNavSections
} from '@dcl/one-ui'
import '../../foundations/foundations.css'
import { nativeNavBarCases } from './navbar-bounds-cases'
const params = new URLSearchParams(location.search)
const requested = params.get('case')
const publicNavigation = params.has('public')
  ? decentralandPublicNavSections('events', '/events')
  : undefined
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
    accountMenuAppearance={params.has('profile') ? 'profile' : undefined}
    sections={publicNavigation?.desktop ?? decentralandNavSections('', '/')}
    mobileSections={publicNavigation?.mobile}
    balances={example.balances}
    surface={example.surface}
    balanceAction={
      example.composition && example.signedIn ? (
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
              <NavMenuItem
                href="/profile/photos"
                role={undefined}
                aria-current="page"
                label="Photos"
                onClick={closeDrawer}
              />
              <NavMenuItem
                href="/profile/places"
                role={undefined}
                label="Places"
                onClick={closeDrawer}
              />
            </nav>
          )
        : undefined
    }
  />
)
