import { useEffect } from 'react'
import { useCommittee } from '../../hooks/useCommittee'
import { useWallet } from '../../store/wallet'

/** Loads the wallet and committee only after the navigation becomes idle. */
export function CurationAvailability({ onChange }: { onChange: (isCurator: boolean) => void }) {
  const address = useWallet(state => state.session?.address)
  const restore = useWallet(state => state.restore)
  useEffect(() => restore(), [restore])
  const { isCurator } = useCommittee(address)
  useEffect(() => onChange(isCurator), [isCurator, onChange])
  return null
}
