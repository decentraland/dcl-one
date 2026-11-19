import { useMemo, type ReactNode } from 'react'
import { ItemThumbnail } from '../ItemThumbnail'
import {
  ClockIcon,
  FilmReelIcon,
  ImageIcon,
  CameraAltIcon as CameraIcon,
  ChangeHistoryIcon as TriangleIcon,
  CircleIcon as MaterialIcon,
  ReportProblemOutlinedIcon as WarningIcon,
  SentimentSatisfiedAltIcon as SmileIcon,
  TextureIcon,
  ThirtyFpsSelectIcon as FpsIcon
} from '../Icons'
import { RequiredPermissions } from '../RequiredPermissions'
import { Tooltip } from '../Tooltip'
import { useThumbnailValidation } from '../../hooks/useThumbnailValidation'
import { useTranslation } from '@/data/lib/creators/intl'
import { THUMBNAIL_PATH, hasFacialExpressions } from '@/data/lib/creators/itemFiles'
import { ItemType } from '@/data/lib/creators/items'
import { isThumbnailGenerating, type PreviewDraft } from '@/data/lib/creators/draftPreview'
import { cx } from '@/data/lib/creators/cx'
import styles from './ItemDraftModal.module.css'

type Props = {
  draft: PreviewDraft
  /** Shows the thumbnail as a plain preview, not an edit control (a variant's target keeps its own). */
  thumbnailLocked?: boolean
  onOpenThumbnail: () => void
  /** Extra controls under the metrics, before the warnings. */
  children?: ReactNode
}

/** The left pane of the item details modals: thumbnail, badges, metrics, permissions and validation warnings. */
export function DraftPreviewPane({ draft, thumbnailLocked = false, onOpenThumbnail, children }: Props) {
  const { t } = useTranslation()
  const isEmote = draft.type === ItemType.EMOTE
  const isWearable = draft.type === ItemType.WEARABLE
  const isSmart = isWearable && draft.isSmart
  const isGenerating = isThumbnailGenerating(draft)

  const thumbnailBlob = draft.contents[THUMBNAIL_PATH]
  const thumbnailSource = useMemo(
    () => (thumbnailBlob ? ({ kind: 'blob', blob: thumbnailBlob } as const) : null),
    [thumbnailBlob]
  )
  const { data: thumbnailValidation } = useThumbnailValidation(thumbnailSource)
  const warnings = useMemo(
    () =>
      [...draft.validationIssues, ...(thumbnailValidation?.issues ?? [])].map(issue => ({
        key: `${issue.code}-${issue.where ?? ''}-${issue.message}`,
        text: issue.message
      })),
    [draft.validationIssues, thumbnailValidation]
  )

  return (
    <div className={styles.previewPane} data-testid="draft-preview-pane">
      <div className={styles.thumbnailWrap}>
        {thumbnailLocked ? (
          <div className={styles.thumbnailBox}>
            <ItemThumbnail
              src={draft.thumbnail}
              rarity={draft.rarity}
              testId="draft-thumbnail"
              loading={isGenerating}
            />
          </div>
        ) : (
          <button
            className={styles.thumbnailBox}
            type="button"
            aria-label={t('add_items_modal.edit_thumbnail')}
            data-testid="edit-thumbnail"
            onClick={onOpenThumbnail}
          >
            <ItemThumbnail src={draft.thumbnail} rarity={draft.rarity} testId="draft-thumbnail" loading={isGenerating}>
              <span className={styles.thumbnailOverlay} data-thumb-overlay>
                <CameraIcon />
              </span>
            </ItemThumbnail>
          </button>
        )}
        {isWearable && hasFacialExpressions(draft.contents) && (
          // The tooltip wraps its trigger in a positioned span, so the corner placement sits outside it.
          <span className={styles.thumbnailBadgeSlot}>
            <Tooltip content={t('add_items_modal.facial_expressions')} asChild>
              <span
                className={styles.thumbnailBadge}
                tabIndex={0}
                aria-label={t('add_items_modal.facial_expressions')}
                data-testid="facial-expressions-badge"
              >
                <SmileIcon />
              </span>
            </Tooltip>
          </span>
        )}
      </div>
      <div
        className={styles.metricsRow}
        data-testid="draft-metrics"
        data-compact={isEmote || undefined}
        aria-busy={!draft.metrics || undefined}
      >
        {isEmote && draft.metrics ? (
          <>
            <span className={styles.metricPill}>
              <FilmReelIcon />
              {t('add_items_modal.metrics.sequences', { count: draft.metrics.sequences ?? 0 })}
            </span>
            <span className={styles.metricPill}>
              <ClockIcon />
              {t('add_items_modal.metrics.duration', { count: Math.round((draft.metrics.duration ?? 0) * 10) / 10 })}
            </span>
            <span className={styles.metricPill}>
              <ImageIcon />
              {t('add_items_modal.metrics.frames', { count: draft.metrics.frames ?? 0 })}
            </span>
            <span className={styles.metricPill}>
              <FpsIcon />
              {t('add_items_modal.metrics.fps', { count: Math.round((draft.metrics.fps ?? 0) * 10) / 10 })}
            </span>
          </>
        ) : draft.metrics ? (
          <>
            <span className={styles.metricPill}>
              <TriangleIcon />
              {t('add_items_modal.metrics.triangles', { count: draft.metrics.triangles ?? 0 })}
            </span>
            <span className={styles.metricPill}>
              <MaterialIcon />
              {t('add_items_modal.metrics.materials', { count: draft.metrics.materials ?? 0 })}
            </span>
            <span className={styles.metricPill}>
              <TextureIcon />
              {t('add_items_modal.metrics.textures', { count: draft.metrics.textures ?? 0 })}
            </span>
          </>
        ) : (
          [0, 1, 2].map(i => (
            <span className={cx(styles.metricSkeleton, 'skeleton')} key={i} data-testid="draft-metrics-loading" />
          ))
        )}
      </div>
      {isSmart && <RequiredPermissions permissions={draft.requiredPermissions} testId="draft-permissions" />}
      {children}
      {warnings.length > 0 && (
        <div className={styles.warningsList} data-testid="draft-warnings">
          {warnings.map(warning => (
            <div className={styles.warningCard} key={warning.key}>
              <WarningIcon />
              {warning.text}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
