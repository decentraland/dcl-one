import { Link, useLocation } from 'react-router'
import { useCommittee } from '@/features/creators/hooks/useCommittee'
import { useWallet } from '@/features/creators/store/wallet'
import { useTranslation } from '@/data/lib/creators/intl'
import styles from './SubNav.module.css'

/** The Create section's tabs: the pages of this section, and the ones Sites serves beside it. */
export function SubNav() {
  const { t } = useTranslation()
  const address = useWallet(state => state.session?.address)
  const { isCurator } = useCommittee(address)
  const { pathname } = useLocation()
  const current = (path: string) => (pathname === path || pathname.startsWith(`${path}/`) ? 'page' : undefined)

  return (
    <div className={styles.subnav} data-testid="subnav">
      <nav className={styles.tabs} data-testid="subnav-tabs">
        <Link to="/create">{t('nav.overview')}</Link>
        <Link to="/create/collections" aria-current={current('/create/collections')}>
          {t('nav.collections')}
        </Link>
        {isCurator && (
          <Link to="/create/curation" aria-current={current('/create/curation')} data-testid="nav-curation">
            {t('nav.curation')}
          </Link>
        )}
        <Link to="/create/scenes">{t('nav.scenes')}</Link>
        <Link to="/create/land">{t('nav.land')}</Link>
      </nav>
    </div>
  )
}
