import { useState } from 'react'
import { BodyShape } from '@dcl/schemas'
import {
  ErrorOutlineIcon as WarningIcon,
  RefreshIcon as RerunIcon,
  ReportProblemOutlinedIcon as ErrorIcon
} from '../Icons'
import { Tooltip } from '../Tooltip'
import { useTranslation } from '@/data/lib/creators/intl'
import type { Item, ItemType } from '@/data/lib/creators/items'
import { ValidationSeverity, type ValidationIssue } from '@/data/lib/creators/validation'
import { ItemFileThumbnail, ItemThumbnail } from '../ItemThumbnail'
import styles from './ItemValidationCard.module.css'

/** The item the results are about, shown above them. */
export type ValidationSubject = {
  name: string
  type: ItemType
  category?: string
  /** Omitted (not null) when there is no artwork tile at all, e.g. a live preview; a saved item's names its stored file. */
  thumbnail?: string | null | { item: Pick<Item, 'isPublished'>; hash: string }
  /** Tints the artwork tile like everywhere else the item shows. */
  rarity?: string | null
}

type Props = {
  subject?: ValidationSubject
  issues: ValidationIssue[]
  /** Checks the item again; the card shows a spinner until it settles. */
  onRerun?: () => Promise<unknown>
  /** Makes the subject a button, e.g. to open the item. */
  onSelect?: () => void
  testId?: string
}

function shapeLabel(shapes: BodyShape[] | undefined): 'male' | 'female' | null {
  if (!shapes || shapes.length !== 1) return null
  return shapes[0] === BodyShape.MALE ? 'male' : 'female'
}

/** One item's validation results: what it is, its issues (or that it passes) and a way to check it again. */
export function ItemValidationCard({ subject, issues, onRerun, onSelect, testId = 'item-validation' }: Props) {
  const { t } = useTranslation()
  const [isRunning, setRunning] = useState(false)

  function rerun() {
    if (!onRerun || isRunning) return
    setRunning(true)
    void onRerun().finally(() => setRunning(false))
  }

  const Subject = onSelect ? 'button' : 'div'

  const rerunButton = onRerun && (
    <Tooltip content={t(isRunning ? 'item_validation.rerunning' : 'item_validation.rerun')} asChild>
      <button
        className={styles.rerun}
        type="button"
        aria-label={t('item_validation.rerun_label')}
        aria-disabled={isRunning || undefined}
        aria-busy={isRunning || undefined}
        data-running={isRunning || undefined}
        data-testid={`${testId}-rerun`}
        onClick={rerun}
      >
        <RerunIcon />
      </button>
    </Tooltip>
  )

  return (
    <section className={styles.card} data-testid={`${testId}-card`}>
      {(subject || rerunButton) && (
        <div className={styles.header}>
          {subject && (
            <Subject
              className={onSelect ? styles.subjectButton : styles.subject}
              {...(onSelect && { type: 'button' as const, 'data-testid': `${testId}-select`, onClick: onSelect })}
            >
              {typeof subject.thumbnail === 'object' && subject.thumbnail ? (
                <ItemFileThumbnail
                  className={styles.thumbnail}
                  item={subject.thumbnail.item}
                  hash={subject.thumbnail.hash}
                  rarity={subject.rarity}
                />
              ) : (
                subject.thumbnail !== undefined && (
                  <ItemThumbnail className={styles.thumbnail} src={subject.thumbnail} rarity={subject.rarity} />
                )
              )}
              <span className={styles.subjectText} data-testid={`${testId}-subject`}>
                <span className={styles.subjectName}>{subject.name}</span>
                <span className={styles.subjectMeta}>
                  <span>{t(`add_items_modal.type.${subject.type}`)}</span>
                  {subject.category && <span>{t(`collection_detail_page.category.${subject.category}`)}</span>}
                </span>
              </span>
            </Subject>
          )}
          {rerunButton}
        </div>
      )}
      {isRunning ? null : issues.length > 0 ? (
        <ul className={styles.issueList} data-testid={`${testId}-issues`}>
          {issues.map((issue, index) => {
            const shape = shapeLabel(issue.bodyShapes)
            return (
              <li
                className={styles.issue}
                key={`${issue.code}-${issue.where ?? ''}-${index}`}
                data-severity={issue.severity}
                data-code={issue.code}
              >
                {issue.severity === ValidationSeverity.ERROR ? <ErrorIcon /> : <WarningIcon />}
                <span className={styles.issueText}>
                  {(issue.title || shape) && (
                    <span className={styles.issueTitle}>
                      {issue.title && <strong>{issue.title}</strong>}
                      {shape && (
                        <span className={styles.shapeLabel} data-testid={`${testId}-shape`}>
                          {t(`item_validation.body_shape.${shape}`)}
                        </span>
                      )}
                    </span>
                  )}
                  {issue.message}
                </span>
              </li>
            )
          })}
        </ul>
      ) : (
        <p className={styles.empty} data-testid={`${testId}-pass`}>
          {t('item_editor.validation.pass')}
        </p>
      )}
    </section>
  )
}
