import { useMemo } from 'react'
import {
  ChevronRightIcon,
  ErrorOutlineIcon as WarningIcon,
  ReportProblemOutlinedIcon as ErrorIcon
} from '../../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { hasErrors as anyErrors, type ItemCheck } from '@/data/lib/creators/validation'
import { Button } from '../../Button'
import { Modal } from '../../Modal'
import { ItemValidationCard, toValidationSubject } from '../../ValidationBadge'
import { type ValidationFlow } from './ValidationGate'
import styles from './ValidationIssuesView.module.css'

type Props = {
  checks: ItemCheck[]
  flow: ValidationFlow
  blockOnErrors: boolean
  onRerun: (check: ItemCheck) => Promise<unknown>
  onBack: () => void
  onContinue: () => void
}

/** The items whose checks found something, before the publish wizard; copy and footer follow the current results. */
export function ValidationIssuesView({ checks, flow, blockOnErrors, onRerun, onBack, onContinue }: Props) {
  const { t } = useTranslation()
  const hasErrors = useMemo(() => checks.some(check => anyErrors(check.issues)), [checks])
  const variant = hasErrors ? (blockOnErrors ? 'errors_blocking' : 'errors') : 'warnings'
  const canContinue = variant !== 'errors_blocking'

  return (
    <Modal
      title={t(`item_validation.issues.${variant}.title`)}
      size="large"
      hideTitle
      showClose
      onClose={onBack}
      testId="publish-validation-issues"
    >
      <div className={styles.content} data-variant={variant} data-testid="publish-validation-issues-content">
        <div className={styles.header}>
          <span className={styles.headerIcon} data-variant={hasErrors ? 'errors' : 'warnings'} aria-hidden>
            {hasErrors ? <ErrorIcon /> : <WarningIcon />}
          </span>
          <h2 className={styles.title} data-testid="publish-validation-issues-title">
            {t(`item_validation.issues.${variant}.title`)}
          </h2>
        </div>
        <p className={styles.intro}>
          {t(`item_validation.issues.${variant}.${flow === 'push_changes' ? 'intro_push_changes' : 'intro'}`)}
        </p>
        <div className={styles.list}>
          {checks.map(check => (
            <ItemValidationCard
              key={check.item.id}
              subject={toValidationSubject(check.item)}
              issues={check.issues}
              onRerun={() => onRerun(check)}
              testId={`publish-validation-${check.item.id}`}
            />
          ))}
        </div>
        <div className={styles.footer} data-single={canContinue ? undefined : ''}>
          <Button
            type="button"
            variant={canContinue ? 'secondary' : 'primary'}
            data-testid="publish-validation-back"
            onClick={onBack}
          >
            {t('item_validation.issues.back')}
          </Button>
          {canContinue && (
            <Button type="button" variant="primary" data-testid="publish-validation-continue" onClick={onContinue}>
              {t('item_validation.issues.continue')}
              <ChevronRightIcon />
            </Button>
          )}
        </div>
      </div>
    </Modal>
  )
}
