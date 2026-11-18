import { useMemo } from 'react'
import { useIntl } from 'react-intl'
import {
  CheckCircleOutlineIcon as PassedIcon,
  ErrorOutlineIcon as WarningIcon,
  HelpOutlineIcon as UncheckedIcon,
  ReportProblemOutlinedIcon as IssuesIcon
} from '@/features/creators/components/Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { Modal } from '@/features/creators/components/Modal'
import { ItemValidationCard } from '@/features/creators/components/ValidationBadge'
import { getContentsStorageUrl } from '@/data/lib/creators/builder'
import {
  countFailedItems,
  groupVerdictItems,
  toCuratorIssues,
  type CuratorVerdict,
  type ValidationItemResult
} from '@/data/lib/creators/events'
import { type Item } from '@/data/lib/creators/items'
import { formatTimeAgo } from '@/data/lib/creators/time'
import styles from './AutomatedVerdictModal.module.css'

type Props = {
  verdict: CuratorVerdict
  /** The creator asked for a curator after the automated review. */
  requestedByCreator?: boolean
  items: Item[]
  onClose: () => void
}

/** The validator's latest verdict for the curator: failed items open first, the rest folded by outcome. */
export function AutomatedVerdictModal({ verdict, requestedByCreator = false, items, onClose }: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const results = verdict.payload.items
  const outcome = verdict.outcome
  const groups = useMemo(() => groupVerdictItems(results), [results])
  const byId = useMemo(() => new Map(items.map(item => [item.id, item])), [items])
  // A section only helps tell outcomes apart: one item, or items that all share an outcome, are listed as they are.
  const flat =
    results.length === 1 || [groups.failed, groups.unchecked, groups.passed].filter(g => g.length).length === 1

  const row = (result: ValidationItemResult) => (
    <ItemRow
      key={result.itemId}
      result={result}
      item={byId.get(result.itemId)}
      defaultOpen={results.length === 1 || result.passed === false}
    />
  )

  return (
    <Modal
      title={t('automated_verdict.title')}
      onClose={onClose}
      size="medium"
      compact
      testId="automated-verdict-modal"
    >
      <div className={styles.body}>
        <div className={styles.banner} data-outcome={outcome} data-testid="automated-verdict-headline">
          <p className={styles.bannerLine}>
            {outcome === 'passed' ? (
              <PassedIcon aria-hidden />
            ) : outcome === 'rejected' ? (
              <IssuesIcon aria-hidden />
            ) : (
              <UncheckedIcon aria-hidden />
            )}
            <span>
              {outcome === 'passed'
                ? t('automated_verdict.banner_passed')
                : outcome === 'rejected'
                  ? t('automated_verdict.banner_issues', { failed: countFailedItems(results), total: results.length })
                  : t('automated_verdict.status_unsupported')}
            </span>
            <span className={styles.time}>{formatTimeAgo(verdict.createdAt, intl.locale)}</span>
          </p>
          {requestedByCreator && (
            <p className={styles.reason} data-testid="automated-verdict-reason">
              {t('automated_verdict.reason_requested')}
            </p>
          )}
        </div>
        <div className={styles.items} data-testid="automated-verdict-items">
          {flat ? (
            results.map(row)
          ) : (
            <>
              {groups.failed.map(row)}
              {groups.unchecked.length > 0 && (
                <details className={styles.group} data-testid="automated-verdict-unchecked">
                  <summary className={styles.groupSummary}>
                    {t('automated_verdict.unchecked_group', { count: groups.unchecked.length })}
                  </summary>
                  <div className={styles.groupItems}>{groups.unchecked.map(row)}</div>
                </details>
              )}
              {groups.passed.length > 0 && (
                <details className={styles.group} data-testid="automated-verdict-passed">
                  <summary className={styles.groupSummary}>
                    {t('automated_verdict.passed_group', { count: groups.passed.length })}
                  </summary>
                  <div className={styles.groupItems}>{groups.passed.map(row)}</div>
                </details>
              )}
            </>
          )}
        </div>
      </div>
    </Modal>
  )
}

type RowProps = { result: ValidationItemResult; item?: Item; defaultOpen: boolean }

/** One item as a box, its outcome as icons at the end of its row. */
function ItemRow({ result, item, defaultOpen }: RowProps) {
  const { t } = useTranslation()
  const issues = useMemo(() => toCuratorIssues(result.findings, item?.contents), [result.findings, item?.contents])
  const errors = useMemo(() => result.findings.filter(finding => finding.severity === 'error').length, [result])
  const warnings = result.findings.length - errors
  const visualRows = useMemo(
    () =>
      (result.visualSummary ?? '')
        .split('\n')
        .map(line => line.trim())
        .filter(Boolean),
    [result.visualSummary]
  )
  const thumbnail = item?.contents[item.thumbnail]
  const outcome = result.passed === true ? 'passed' : result.passed === false ? 'failed' : 'unchecked'
  const unchecked = t(result.unsupported ? 'findings.item_unsupported' : 'findings.item_unchecked')
  const testId = `automated-verdict-items-${result.itemId}`

  return (
    <details className={styles.item} open={defaultOpen || undefined} data-outcome={outcome} data-testid={testId}>
      <summary className={styles.itemSummary}>
        {thumbnail ? (
          <img className={styles.thumb} src={getContentsStorageUrl(thumbnail)} alt="" />
        ) : (
          <span className={styles.thumbPlaceholder} />
        )}
        <span className={styles.itemName}>
          {item?.name ?? t('findings.unknown_item', { id: result.itemId.slice(0, 8) })}
        </span>
        <span className={styles.status} data-testid={`${testId}-status`}>
          {outcome === 'unchecked' ? (
            <span className={styles.count} data-kind="unchecked" title={unchecked} aria-label={unchecked}>
              <UncheckedIcon fontSize="small" />
            </span>
          ) : errors === 0 && warnings === 0 ? (
            <span
              className={styles.count}
              data-kind="passed"
              title={t('findings.item_passed')}
              aria-label={t('findings.item_passed')}
            >
              <PassedIcon fontSize="small" />
            </span>
          ) : (
            <>
              {errors > 0 && (
                <span
                  className={styles.count}
                  data-kind="error"
                  title={t('automated_verdict.errors', { count: errors })}
                  aria-label={t('automated_verdict.errors', { count: errors })}
                >
                  <IssuesIcon fontSize="small" />
                  {errors}
                </span>
              )}
              {warnings > 0 && (
                <span
                  className={styles.count}
                  data-kind="warning"
                  title={t('automated_verdict.warnings', { count: warnings })}
                  aria-label={t('automated_verdict.warnings', { count: warnings })}
                >
                  <WarningIcon fontSize="small" />
                  {warnings}
                </span>
              )}
            </>
          )}
        </span>
      </summary>
      <div className={styles.itemBody}>
        {result.passed === null && result.error && (
          <p className={styles.note}>{t('findings.item_error', { error: result.error })}</p>
        )}
        {visualRows.length > 0 && (
          <div className={styles.visual} data-testid="automated-verdict-items-visual-summary">
            <span className={styles.visualTitle}>{t('findings.visual_summary')}</span>
            {visualRows.map((line, index) => (
              <p className={styles.visualLine} key={index}>
                {line}
              </p>
            ))}
          </div>
        )}
        {(issues.length > 0 || outcome !== 'unchecked') && <ItemValidationCard issues={issues} testId={testId} />}
      </div>
    </details>
  )
}
