import { useCallback, useEffect, useMemo } from 'react'
import { NavLink, useLocation } from 'react-router-dom'
import { Network } from '@dcl/schemas/dist/dapps/network'
import { TopNav } from '~/components/TopNav'
import { useWallet } from '~/store/wallet'
import { useCreditsBalance, useManaBalance } from '~/hooks/useBalances'
import { useCommittee } from '~/hooks/useCommittee'
import { useProfile } from '~/hooks/useProfile'
import { useWalletChain } from '~/hooks/useWalletChain'
import { track } from '~/lib/analytics'
import { isSocialLogin } from '~/lib/auth'
import { openExternal } from '~/lib/navigation'
import { isOverviewPath, LandingPlace, trackLandingClick } from '~/lib/overviewAnalytics'
import { useTranslation } from '~/intl'
import { config } from '~/config'
import styles from './NavBar.module.css'

// Scenes and Land live in the legacy builder web app, not in this SPA, so both are plain same-tab links.
const builderUrl = config.get('BUILDER_URL')
const shopCreditsUrl = `${config.get('SHOP_URL')}/credits`
const accountUrl = config.get('ACCOUNT_URL')

// Parsed from the exact decimal string, as viem's formatEther would give it, without loading viem on the overview.
function weiToMana(wei: bigint): number {
  const digits = wei.toString().padStart(19, '0')
  return Number(`${digits.slice(0, -18)}.${digits.slice(-18)}`)
}

const NavBar = () => {
  const { t } = useTranslation()
  const { session, connecting, signIn, disconnect } = useWallet()
  const address = session?.address
  const { data: avatar, isLoading: isLoadingProfile } = useProfile(address)
  const { data: credits } = useCreditsBalance(address)
  const { data: manaWei } = useManaBalance(address)
  const { isCurator } = useCommittee(address)
  // A social login has no network to choose (every write of theirs is relayed), so it is handed no session
  // and ui2 hides the chain pill inside the profile panel. The event is the legacy builder's.
  const { chainId, chains, switchTo } = useWalletChain(session && !isSocialLogin(session) ? session : null)
  const selectChain = useCallback(
    (chain: number) => {
      track('Change Network', { from_chain_id: chainId, to_chain_id: chain })
      void switchTo(chain)
    },
    [chainId, switchTo]
  )
  // Polygon only: it's the network publishing pays on. Like the legacy builder, an empty wallet shows
  // no MANA chip at all; ui2 renders whole units.
  const manaBalances = useMemo(() => (manaWei ? { [Network.MATIC]: weiToMana(manaWei) } : undefined), [manaWei])
  const { pathname } = useLocation()
  // Collections stays active across the collection detail / item detail / editor pages too, not just
  // the /collections list — a NavLink to /collections alone wouldn't light up on the nested routes.
  const collectionsActive = /^\/collections(\/|$)/.test(pathname)
  // The top bar's translucent fill washes out over light content, so it deepens once the page scrolls.
  // Set on <body> because the ui2 navbar is styled from TopNav via ancestor selectors.
  useEffect(() => {
    const onScroll = () => {
      document.body.toggleAttribute('data-scrolled', window.scrollY > 8)
    }
    onScroll()
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      window.removeEventListener('scroll', onScroll)
      document.body.removeAttribute('data-scrolled')
    }
  }, [])

  return (
    <>
      <TopNav
        activePage="create"
        isSignedIn={!!session}
        isSigningIn={connecting}
        isLoadingProfile={!!session && isLoadingProfile}
        address={address}
        avatar={avatar}
        shopCreditsBalance={credits?.credits}
        onClickShopCredits={() => openExternal(shopCreditsUrl)}
        manaBalances={manaBalances}
        showManaBalancesInNavbar
        onClickBalance={() => openExternal(accountUrl)}
        selectedChain={chainId}
        chains={chains}
        onSelectChain={selectChain}
        onClickSignIn={() => {
          if (isOverviewPath(pathname)) trackLandingClick(LandingPlace.NAVBAR, { action: 'sign_in' })
          signIn()
        }}
        onClickSignOut={() => void disconnect()}
      />

      <div className={styles.subnav} data-testid="subnav">
        <nav className={styles.tabs} data-testid="subnav-tabs">
          <NavLink to="/" end>
            {t('nav.overview')}
          </NavLink>
          <NavLink to="/collections" className={() => (collectionsActive ? 'active' : '')}>
            {t('nav.collections')}
          </NavLink>
          {isCurator && (
            <NavLink to="/curation" data-testid="nav-curation">
              {t('nav.curation')}
            </NavLink>
          )}
          <a href={`${builderUrl}/scenes`}>{t('nav.scenes')}</a>
          <a href={`${builderUrl}/land`}>{t('nav.land')}</a>
        </nav>
      </div>
    </>
  )
}

export { NavBar }
