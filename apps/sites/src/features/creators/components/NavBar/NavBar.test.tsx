import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import type { NavbarProps } from 'decentraland-ui2'
import { TranslationProvider } from '~/intl'
import { config } from '~/config'
import { openExternal } from '~/lib/navigation'
import { sendOverviewTrack } from '~/lib/overviewSegment'
import { track } from '~/lib/analytics'
import { NavBar } from './NavBar'

// Stands in for the ui2 navbar: surfaces the balance chips the real one renders from these props.
vi.mock('~/components/TopNav', () => ({
  TopNav: ({
    shopCreditsBalance,
    onClickShopCredits,
    manaBalances,
    onClickBalance,
    onClickSignIn,
    selectedChain,
    chains,
    onSelectChain
  }: NavbarProps) => (
    <div data-testid="topnav">
      {selectedChain !== undefined &&
        chains?.map(chain => (
          <button
            key={chain}
            data-testid={`topnav-chain-${chain}`}
            data-selected={chain === selectedChain}
            onClick={() => onSelectChain?.(chain)}
          >
            {chain}
          </button>
        ))}
      <button data-testid="topnav-sign-in" onClick={onClickSignIn}>
        sign in
      </button>
      {shopCreditsBalance !== undefined && (
        <button data-testid="topnav-credits" onClick={onClickShopCredits}>
          {shopCreditsBalance}
        </button>
      )}
      {manaBalances?.MATIC !== undefined && (
        <button data-testid="topnav-mana" onClick={() => onClickBalance?.('MATIC' as never)}>
          {manaBalances.MATIC}
        </button>
      )}
    </div>
  )
}))

const wallet = vi.hoisted(() => ({
  session: undefined as { address: string; providerType?: string } | undefined,
  signIn: vi.fn()
}))
vi.mock('~/store/wallet', () => ({
  useWallet: () => ({
    session: wallet.session,
    connecting: false,
    signIn: wallet.signIn,
    disconnect: vi.fn(),
    restore: vi.fn()
  })
}))

vi.mock('~/hooks/useProfile', () => ({
  useProfile: () => ({ data: undefined, isLoading: false })
}))

const balances = vi.hoisted(() => ({
  credits: undefined as number | undefined,
  manaWei: undefined as bigint | undefined
}))
vi.mock('~/hooks/useBalances', () => ({
  useCreditsBalance: () => ({ data: balances.credits === undefined ? undefined : { credits: balances.credits } }),
  useManaBalance: () => ({ data: balances.manaWei })
}))

vi.mock('~/lib/navigation', () => ({ openExternal: vi.fn() }))
vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))
vi.mock('~/lib/auth', () => ({
  isSocialLogin: (session: { providerType?: string }) => session.providerType === 'magic'
}))

const walletChain = vi.hoisted(() => ({
  hook: vi.fn(),
  switchTo: vi.fn()
}))
vi.mock('~/hooks/useWalletChain', () => ({
  useWalletChain: (session: unknown) => {
    walletChain.hook(session)
    return session
      ? { chainId: 80002, chains: [80002, 11155111], switchTo: walletChain.switchTo }
      : { chainId: undefined, chains: [80002, 11155111], switchTo: walletChain.switchTo }
  }
}))
vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))

const committee = vi.hoisted(() => ({ isCurator: false }))
vi.mock('~/hooks/useCommittee', () => ({ useCommittee: () => ({ isCurator: committee.isCurator }) }))

beforeEach(() => {
  committee.isCurator = false
  wallet.session = undefined
  balances.credits = undefined
  balances.manaWei = undefined
  vi.mocked(openExternal).mockReset()
  vi.mocked(sendOverviewTrack).mockClear()
  wallet.signIn.mockClear()
  walletChain.hook.mockClear()
  walletChain.switchTo.mockClear()
  vi.mocked(track).mockClear()
})

function renderNavBar(path = '/collections') {
  return render(
    <TranslationProvider>
      <MemoryRouter initialEntries={[path]}>
        <NavBar />
      </MemoryRouter>
    </TranslationProvider>
  )
}

describe('NavBar', () => {
  it("reports a sign-in from the overview the way sites' landing navbar did", () => {
    renderNavBar('/')
    fireEvent.click(screen.getByTestId('topnav-sign-in'))
    expect(wallet.signIn).toHaveBeenCalled()
    expect(sendOverviewTrack).toHaveBeenCalledWith('Click', {
      place: 'Landing Navbar',
      event: 'click',
      action: 'sign_in'
    })
  })

  it('signs in without reporting it to the landing tables elsewhere', () => {
    renderNavBar('/collections')
    fireEvent.click(screen.getByTestId('topnav-sign-in'))
    expect(wallet.signIn).toHaveBeenCalled()
    expect(sendOverviewTrack).not.toHaveBeenCalled()
  })

  it('shows the section tabs, keeping Overview and Collections in-app and sending Scenes and Land to the legacy builder', () => {
    renderNavBar()
    expect(screen.getByRole('link', { name: 'Overview' })).toHaveAttribute('href', '/')
    expect(screen.getByRole('link', { name: 'Collections' })).toHaveAttribute('href', '/collections')
    expect(screen.getByRole('link', { name: 'Scenes' })).toHaveAttribute(
      'href',
      'https://decentraland.zone/builder/scenes'
    )
    expect(screen.getByRole('link', { name: 'Land/World' })).toHaveAttribute(
      'href',
      'https://decentraland.zone/builder/land'
    )
  })

  it('shows the Curation tab to committee members only', () => {
    renderNavBar()
    expect(screen.queryByRole('link', { name: 'Curation' })).toBeNull()

    committee.isCurator = true
    renderNavBar('/curation')
    expect(screen.getByRole('link', { name: 'Curation' })).toHaveAttribute('href', '/curation')
    expect(screen.getByRole('link', { name: 'Curation' })).toHaveAttribute('aria-current', 'page')
  })

  it('keeps the Collections tab active on nested collection routes', () => {
    renderNavBar('/collections/a1b2')
    expect(screen.getByRole('link', { name: 'Collections' })).toHaveAttribute('aria-current', 'page')
    expect(screen.getByRole('link', { name: 'Overview' })).not.toHaveAttribute('aria-current')
  })

  it('lights up Overview on the home route only', () => {
    renderNavBar('/')
    expect(screen.getByRole('link', { name: 'Overview' })).toHaveAttribute('aria-current', 'page')
    expect(screen.getByRole('link', { name: 'Collections' })).not.toHaveAttribute('aria-current')
  })

  it('flags a scrolled page on <body> so the top bar deepens, and clears it on unmount', () => {
    const scrollTo = (y: number) => {
      Object.defineProperty(window, 'scrollY', { value: y, writable: true, configurable: true })
      fireEvent.scroll(window)
    }
    const { unmount } = renderNavBar()
    expect(document.body).not.toHaveAttribute('data-scrolled')

    scrollTo(20)
    expect(document.body).toHaveAttribute('data-scrolled')

    scrollTo(0)
    expect(document.body).not.toHaveAttribute('data-scrolled')

    scrollTo(20)
    unmount()
    expect(document.body).not.toHaveAttribute('data-scrolled')

    scrollTo(20)
    expect(document.body).not.toHaveAttribute('data-scrolled')
    scrollTo(0)
  })

  it('shows the credits and MANA balances in the top bar, linking out to buy more', () => {
    wallet.session = { address: '0xabc' }
    balances.credits = 42
    balances.manaWei = 1_234_500_000_000_000_000_000n
    renderNavBar()

    expect(screen.getByTestId('topnav-credits')).toHaveTextContent('42')
    expect(screen.getByTestId('topnav-mana')).toHaveTextContent('1234.5')

    fireEvent.click(screen.getByTestId('topnav-credits'))
    expect(openExternal).toHaveBeenCalledWith(`${config.get('SHOP_URL')}/credits`)
    fireEvent.click(screen.getByTestId('topnav-mana'))
    expect(openExternal).toHaveBeenCalledWith(config.get('ACCOUNT_URL'))
  })

  it('lets a creator with a wallet see and change the network from the profile panel', () => {
    wallet.session = { address: '0xabc', providerType: 'injected' }
    renderNavBar()
    expect(walletChain.hook).toHaveBeenCalledWith(expect.objectContaining({ address: '0xabc' }))
    expect(screen.getByTestId('topnav-chain-80002')).toHaveAttribute('data-selected', 'true')
    expect(screen.getByTestId('topnav-chain-11155111')).toHaveAttribute('data-selected', 'false')

    fireEvent.click(screen.getByTestId('topnav-chain-11155111'))
    expect(walletChain.switchTo).toHaveBeenCalledWith(11155111)
    expect(track).toHaveBeenCalledWith('Change Network', { from_chain_id: 80002, to_chain_id: 11155111 })
  })

  it('shows no network pill to a social login, which has none to choose', () => {
    wallet.session = { address: '0xabc', providerType: 'magic' }
    renderNavBar()
    expect(screen.queryByTestId(/^topnav-chain-/)).toBeNull()
  })

  it('shows no network pill while signed out', () => {
    renderNavBar()
    expect(screen.queryByTestId(/^topnav-chain-/)).toBeNull()
  })

  it('shows no balance chips until each balance has loaded', () => {
    wallet.session = { address: '0xabc' }
    renderNavBar()
    expect(screen.queryByTestId('topnav-credits')).toBeNull()
    expect(screen.queryByTestId('topnav-mana')).toBeNull()
  })

  it('shows zero credits but hides the MANA chip for an empty wallet', () => {
    wallet.session = { address: '0xabc' }
    balances.credits = 0
    balances.manaWei = 0n
    renderNavBar()
    expect(screen.getByTestId('topnav-credits')).toHaveTextContent('0')
    expect(screen.queryByTestId('topnav-mana')).toBeNull()
  })
})
