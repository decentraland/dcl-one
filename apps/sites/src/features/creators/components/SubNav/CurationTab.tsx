import { useEffect } from 'react'
import { Link } from 'react-router'
import { useCommittee } from '../../hooks/useCommittee'
import { useWallet } from '../../store/wallet'

/** The tab to the curation queue, for a signed-in member of the curation committee. */
export function CurationTab({ current }: { current: 'page' | undefined }) {
  const address = useWallet(state => state.session?.address)
  const restore = useWallet(state => state.restore)
  useEffect(() => restore(), [restore])
  const { isCurator } = useCommittee(address)

  if (!isCurator) return null
  return (
    <Link to="/create/curation" aria-current={current} data-testid="nav-curation">
      Curation
    </Link>
  )
}
