import { useTranslation } from '~/intl'
import styles from './RequiredPermissions.module.css'

type Props = {
  permissions: string[]
  testId?: string
}

const DOCS_URL =
  'https://docs.decentraland.org/creator/scenes-sdk7/kinds-of-projects/scene-metadata#required-permissions'

/** Read-only list of the scene permissions a smart wearable declares (legacy ItemRequiredPermission). */
export function RequiredPermissions({ permissions, testId = 'required-permissions' }: Props) {
  const { t } = useTranslation()
  return (
    <div className={styles.wrap} data-testid={testId}>
      <span className={styles.title}>
        {t('required_permissions.title')}
        <a className={styles.docsLink} href={DOCS_URL} target="_blank" rel="noreferrer">
          {t('required_permissions.learn_more')}
        </a>
      </span>
      {permissions.length === 0 ? (
        <span>{t('required_permissions.none')}</span>
      ) : (
        <ul className={styles.list}>
          {permissions.map(permission => (
            <li className={styles.chip} key={permission} data-testid={`${testId}-${permission}`}>
              {t(`required_permissions.permission.${permission}`)}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
