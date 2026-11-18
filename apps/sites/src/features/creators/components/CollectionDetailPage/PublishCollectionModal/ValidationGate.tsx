import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from '~/intl'
import { useRerunItemValidation } from '~/hooks/useCollectionValidation'
import { useCatalystItemSizes } from '~/hooks/useItemSize'
import { track } from '~/lib/analytics'
import { formatItemSize, isOverSizeLimit } from '~/lib/itemSize'
import { ValidationSeverity, countIssues, getValidationStatus, hasErrors, type ValidationIssue } from '~/lib/validation'
import { PendingModal } from '~/components/PendingModal'
import { type ItemCheck } from '~/lib/validation'
import { ValidationIssuesView } from './ValidationIssuesView'

export type ValidationFlow = 'publish' | 'push_changes'

type Props = {
  collectionId: string
  flow: ValidationFlow
  /** The collection's item checks, run by the page; items still loading count as validating. */
  validation: { isValidating: boolean; results: ItemCheck[] }
  /** Errors block the flow (`block-publish-on-validation-errors`): no way past the issues view while any remain. */
  blockOnErrors: boolean
  onPass: () => void
  onClose: () => void
}

const SIZE_ISSUE_CODE = 'item_size'
const SIZE_UNKNOWN_CODE = 'item_size_unknown'
const isSizeIssue = (issue: ValidationIssue) => issue.code === SIZE_ISSUE_CODE || issue.code === SIZE_UNKNOWN_CODE

function totals(checks: ItemCheck[]) {
  return countIssues(checks.flatMap(check => check.issues))
}

/**
 * The pre-step before publishing or sending changes for review: waits for the item checks and the items' sizes, then
 * shows anything they found. Clean results pass straight through; the decision is made once per opening. An item over
 * its size limit always blocks, whatever the flag says: the catalyst would refuse it at deploy.
 */
export function ValidationGate({ collectionId, flow, validation, blockOnErrors, onPass, onClose }: Props) {
  const { t } = useTranslation()
  const [decided, setDecided] = useState(false)
  const openedAt = useRef(Date.now())
  // The items the issues view lists: those with issues when it opened, errors first. A re-run that clears
  // one keeps its card so the creator sees it pass.
  const [flaggedIds, setFlaggedIds] = useState<string[]>([])

  useEffect(() => {
    track('Publish Validation Started', { collectionId, flow, itemCount: validation.results.length })
    // Once per opening.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const items = useMemo(() => validation.results.map(check => check.item), [validation.results])
  const sizes = useCatalystItemSizes(items)
  const checks = useMemo(
    () =>
      validation.results.map(check => {
        // Unmeasured is not clean: the catalyst still counts the size at deploy, so the creator hears it can fail.
        if (sizes.unknown.has(check.item.id)) {
          const unknownIssue = {
            code: SIZE_UNKNOWN_CODE,
            severity: ValidationSeverity.WARNING,
            title: t('item_size.label'),
            message: t('item_size.unknown_warning')
          }
          return { ...check, issues: [...check.issues, unknownIssue] }
        }
        const size = sizes.sizes.get(check.item.id)
        if (!size || !isOverSizeLimit(size)) return check
        const sizeIssue = {
          code: SIZE_ISSUE_CODE,
          severity: ValidationSeverity.ERROR,
          title: t('item_size.label'),
          message: t('add_items_modal.file_error.size_exceeded', formatItemSize(size))
        }
        return { ...check, issues: [sizeIssue, ...check.issues] }
      }),
    [validation.results, sizes.sizes, sizes.unknown, t]
  )
  const oversizedCount = useMemo(
    () => checks.filter(check => check.issues.some(issue => issue.code === SIZE_ISSUE_CODE)).length,
    [checks]
  )
  const blocking = blockOnErrors || oversizedCount > 0

  useEffect(() => {
    if (decided || validation.isValidating || sizes.isMeasuring) return
    const flagged = checks.filter(check => check.issues.length > 0)
    const { errors, warnings } = totals(flagged)
    track('Publish Validation Result', {
      collectionId,
      flow,
      errors,
      warnings,
      itemsWithIssues: flagged.length,
      oversizedItems: oversizedCount,
      blocking: blocking && errors > 0,
      durationMs: Date.now() - openedAt.current
    })
    setDecided(true)
    if (flagged.length === 0) {
      onPass()
      return
    }
    const withErrors = flagged.filter(check => hasErrors(check.issues))
    const withWarnings = flagged.filter(check => !hasErrors(check.issues))
    setFlaggedIds([...withErrors, ...withWarnings].map(({ item }) => item.id))
  }, [
    decided,
    validation.isValidating,
    sizes.isMeasuring,
    checks,
    oversizedCount,
    blocking,
    collectionId,
    flow,
    onPass
  ])

  const flaggedChecks = useMemo(() => {
    const byId = new Map(checks.map(check => [check.item.id, check]))
    return flaggedIds.flatMap(id => byId.get(id) ?? [])
  }, [flaggedIds, checks])
  const rerunItemValidation = useRerunItemValidation()
  const rerun = useCallback(
    // The re-run reports the rule book's verdict it replaces, without the size issues this step adds.
    (check: ItemCheck) => {
      // A size that could not be measured is measured again with the checks.
      if (sizes.unknown.has(check.item.id)) void sizes.remeasure(check.item)
      return rerunItemValidation(
        check.item,
        flow,
        getValidationStatus(
          check.issues.filter(issue => !isSizeIssue(issue)),
          false
        )
      )
    },
    [rerunItemValidation, flow, sizes]
  )
  function resolveIssues(action: 'continue' | 'back') {
    track('Publish Validation Resolved', { collectionId, flow, action, ...totals(flaggedChecks) })
    if (action === 'continue') onPass()
    else onClose()
  }

  if (!decided) {
    return <PendingModal label={t('item_validation.checking')} onCancel={onClose} testId="publish-validating" />
  }

  return (
    <ValidationIssuesView
      checks={flaggedChecks}
      flow={flow}
      blockOnErrors={blocking}
      onRerun={rerun}
      onBack={() => resolveIssues('back')}
      onContinue={() => resolveIssues('continue')}
    />
  )
}
