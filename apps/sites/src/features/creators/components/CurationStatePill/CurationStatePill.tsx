import { useTranslation } from '~/intl'
import { type CurationState } from '~/lib/curation'
import styles from './CurationStatePill.module.css'

export function CurationStatePill({ state }: { state: CurationState }) {
  const { t } = useTranslation()
  return (
    <span className={styles.pill} data-testid="curation-state" data-state={state}>
      {t(`curation_state.${state}`)}
    </span>
  )
}
