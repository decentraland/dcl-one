import { useProfile } from '~/hooks/useProfile'
import { useTranslation } from '~/intl'
import { shortenAddress } from '~/lib/address'
import styles from './ProfileBadge.module.css'

type Props = {
  address: string
  /** Appends "(you)" when this is the signed-in wallet. */
  self?: boolean
  testId?: string
}

/** A wallet as a face and its Decentraland name, the short address while (or when) there is no profile. */
export function ProfileBadge({ address, self = false, testId = 'profile-badge' }: Props) {
  const { t } = useTranslation()
  const { data: profile } = useProfile(address)
  const face = profile?.avatar?.snapshots?.face256
  const name = profile?.name || shortenAddress(address)
  return (
    <span className={styles.badge} data-testid={testId} title={address}>
      {face ? <img className={styles.face} src={face} alt="" /> : <span className={styles.faceFallback} aria-hidden />}
      <span className={styles.name}>{name}</span>
      {self && <span className={styles.you}>{t('profile_badge.you')}</span>}
    </span>
  )
}
