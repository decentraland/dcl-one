import { useMemo } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { getContentsStorageUrl } from '@/data/lib/creators/builder'
import { mergeBodyShapeFindings, type ValidationFinding, type ValidationItemResult } from '@/data/lib/creators/events'
import type { Item } from '@/data/lib/creators/items'
import { isWebUrl, openExternal } from '@/data/lib/creators/navigation'
import styles from './FindingsList.module.css'

type ShownFinding = Omit<ValidationFinding, 'measured' | 'limit'> & {
  measured?: number | string
  limit?: number | string
}

type ListProps = {
  findings: ShownFinding[]
  testId?: string
}

/** Validator findings, one card each: rule, body shapes, message, measured vs limit, fix and docs. */
export function FindingsList({ findings, testId = 'findings' }: ListProps) {
  const { t } = useTranslation()
  const rows = useMemo(() => mergeBodyShapeFindings(findings), [findings])
  if (rows.length === 0) return null
  return (
    <ul className={styles.list} data-testid={testId}>
      {rows.map((finding, index) => (
        <li
          className={styles.finding}
          key={`${finding.rule}-${index}`}
          data-severity={finding.severity}
          data-testid={`${testId}-finding`}
        >
          <div className={styles.head}>
            <span className={styles.severity} data-severity={finding.severity}>
              {t(`findings.severity.${finding.severity}`)}
            </span>
            <span className={styles.rule}>{finding.rule}</span>
            {finding.bodyShapes.length > 0 && (
              <span className={styles.bodyShapes} data-testid={`${testId}-body-shapes`}>
                {finding.bodyShapes.map(shape => t(`findings.body_shape.${shape}`)).join(' · ')}
              </span>
            )}
            {finding.where && <span className={styles.where}>{finding.where}</span>}
          </div>
          <p className={styles.message}>{finding.message}</p>
          {(finding.measured !== undefined || finding.limit !== undefined) && (
            <p className={styles.meta}>
              {finding.measured !== undefined && <span>{t('findings.measured', { measured: finding.measured })}</span>}
              {finding.limit !== undefined && <span>{t('findings.limit', { limit: finding.limit })}</span>}
            </p>
          )}
          {finding.fix && (
            <p className={styles.fix}>
              <strong>{t('findings.fix')}:</strong> {finding.fix}
            </p>
          )}
          {isWebUrl(finding.docs) && (
            <a
              className={styles.docsLink}
              href={finding.docs}
              onClick={event => {
                event.preventDefault()
                openExternal(finding.docs!)
              }}
            >
              {t('findings.docs')}
            </a>
          )}
        </li>
      ))}
    </ul>
  )
}

type ItemFindings = Pick<ValidationItemResult, 'itemId'> & { findings: ShownFinding[] } & Partial<
    Pick<ValidationItemResult, 'passed' | 'visualSummary' | 'unsupported' | 'error'>
  >

type GroupedProps = {
  results: ItemFindings[]
  /** The collection's items, to show names and thumbnails; unknown ids fall back to a short id. */
  items?: Item[]
  /** Hides the pass/fail label per item (the pre-fee checks have no verdict). */
  showStatus?: boolean
  /** The validator's own failure text for an unchecked item: committee views only, creators see the status. */
  showErrors?: boolean
  testId?: string
}

/** Findings grouped by item, with the item's thumbnail, name and (optionally) its pass/fail and visual summary. */
export function ItemFindingsList({
  results,
  items = [],
  showStatus = true,
  showErrors = false,
  testId = 'item-findings'
}: GroupedProps) {
  const { t } = useTranslation()
  const byId = useMemo(() => new Map(items.map(item => [item.id, item])), [items])
  return (
    <div className={styles.groups} data-testid={testId}>
      {results.map(result => {
        const item = byId.get(result.itemId)
        const thumbnail = item?.contents[item.thumbnail]
        const passed = result.passed === undefined ? undefined : result.passed
        return (
          <section
            className={styles.itemGroup}
            key={result.itemId}
            data-testid={`${testId}-item`}
            data-item-id={result.itemId}
          >
            <header className={styles.itemHead}>
              {thumbnail && <img className={styles.itemThumb} src={getContentsStorageUrl(thumbnail)} alt="" />}
              <span>{item?.name ?? t('findings.unknown_item', { id: result.itemId.slice(0, 8) })}</span>
              {showStatus && passed !== undefined && (
                <span className={styles.itemStatus} data-passed={String(passed)}>
                  {t(
                    passed === true
                      ? 'findings.item_passed'
                      : passed === false
                        ? 'findings.item_failed'
                        : result.unsupported
                          ? 'findings.item_unsupported'
                          : 'findings.item_unchecked'
                  )}
                </span>
              )}
            </header>
            {showErrors && result.passed === null && result.error && (
              <p className={styles.itemError} data-testid={`${testId}-item-error`}>
                {t('findings.item_error', { error: result.error })}
              </p>
            )}
            <FindingsList findings={result.findings} testId={`${testId}-${result.itemId}`} />
            {result.visualSummary && (
              <p className={styles.summary} data-testid={`${testId}-visual-summary`}>
                <strong>{t('findings.visual_summary')}:</strong> {result.visualSummary}
              </p>
            )}
          </section>
        )
      })}
    </div>
  )
}
