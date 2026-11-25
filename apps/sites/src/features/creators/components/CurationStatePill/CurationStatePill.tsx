import { useTranslation } from '@/data/lib/creators/intl'
import type { CurationState } from '@/data/lib/creators/curation'
import styles from './CurationStatePill.module.css'

export function CurationStatePill({ state }: { state: CurationState }) {
  const { t } = useTranslation()
  return (
    <span className={styles.pill} data-testid="curation-state" data-state={state}>
      {t(`curation_state.${state}`)}
    </span>
  )
}
