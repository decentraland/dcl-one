import {
  CheckCircleOutlineIcon as PassedIcon,
  HelpOutlineIcon as UnsupportedIcon,
  ReportProblemOutlinedIcon as IssuesIcon
} from '@/features/creators/components/Icons'
import { type CuratorVerdictOutcome } from '@/data/lib/creators/events'
import styles from './AutomatedReviewIcon.module.css'

export type AutomatedReviewStatus = CuratorVerdictOutcome | 'running'

/** The automated review's outcome as one colored glyph, or a spinner while it runs. */
export function AutomatedReviewIcon({ status }: { status: AutomatedReviewStatus }) {
  return (
    <span className={styles.glyph} data-status={status} aria-hidden>
      {status === 'running' ? (
        <span className={styles.spinner} />
      ) : status === 'passed' ? (
        <PassedIcon fontSize="inherit" />
      ) : status === 'rejected' ? (
        <IssuesIcon fontSize="inherit" />
      ) : (
        <UnsupportedIcon fontSize="inherit" />
      )}
    </span>
  )
}
