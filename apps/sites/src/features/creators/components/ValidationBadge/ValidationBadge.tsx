import { useMemo, useState } from 'react'
import {
  CheckCircleOutlineIcon as PassIcon,
  ErrorOutlineIcon as WarningIcon,
  ReportProblemOutlinedIcon as ErrorIcon
} from '../Icons'
import { Tooltip } from '../Tooltip'
import { useTranslation } from '@/data/lib/creators/intl'
import { type ValidationIssue, type ValidationStatus } from '@/data/lib/creators/validation'
import { type ValidationSubject } from './ItemValidationCard'
import { ValidationResultsModal, type ItemResult } from './ValidationResultsModal'
import styles from './ValidationBadge.module.css'

type Props = {
  status: ValidationStatus
  /** One item's issues; with `results`, they're taken from there. */
  issues?: ValidationIssue[]
  subject?: ValidationSubject
  /** Several items' results, listed in the modal instead of `subject`'s. */
  results?: ItemResult[]
  /** Whose checks the tooltip talks about. */
  scope?: 'item' | 'collection'
  onRerun?: () => Promise<unknown>
  /** Called when the results open, e.g. for analytics. */
  onOpen?: () => void
  testId?: string
}

const TOOLTIP_KEYS = { item: 'item_editor.validation', collection: 'item_editor.review.validation' } as const

/** Traffic light for the selected item's checks; opens the issue list when there is one. */
export function ValidationBadge({
  status,
  issues,
  subject,
  results,
  scope = 'item',
  onRerun,
  onOpen,
  testId = 'validation-badge'
}: Props) {
  const { t } = useTranslation()
  const [isOpen, setOpen] = useState(false)
  const shownIssues = useMemo(() => results?.flatMap(result => result.issues) ?? issues ?? [], [results, issues])
  if (status === 'idle') return null
  const hasIssues = shownIssues.length > 0
  const tooltip = t(`${TOOLTIP_KEYS[scope]}.${status}`)
  return (
    <>
      <Tooltip content={tooltip} asChild testId={`${testId}-tooltip`}>
        <button
          className={styles.badge}
          type="button"
          data-status={status}
          data-testid={testId}
          aria-label={tooltip}
          disabled={!hasIssues}
          onClick={() => {
            if (!hasIssues) return
            setOpen(true)
            onOpen?.()
          }}
        >
          {status === 'loading' && <span className={styles.spinner} aria-hidden />}
          {status === 'pass' && <PassIcon fontSize="small" />}
          {status === 'warnings' && <WarningIcon fontSize="small" />}
          {status === 'errors' && <ErrorIcon fontSize="small" />}
          {t(`item_editor.validation.label.${status}`, { count: shownIssues.length })}
        </button>
      </Tooltip>
      {isOpen && (
        <ValidationResultsModal
          subject={subject}
          issues={shownIssues}
          results={results?.map(result =>
            result.onSelect
              ? {
                  ...result,
                  onSelect: () => {
                    setOpen(false)
                    result.onSelect?.()
                  }
                }
              : result
          )}
          onRerun={onRerun}
          onClose={() => setOpen(false)}
          testId={testId}
        />
      )}
    </>
  )
}
