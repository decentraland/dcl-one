import { useMemo } from 'react'
import { MultiSelect } from '@/features/creators/components/MultiSelect'
import { InfoTooltip } from '@/features/creators/components/Tooltip'
import { useTranslation } from '@/data/lib/creators/intl'
import { getHideableBodyPartCategories, getHideableWearableCategories } from '@/data/lib/creators/wearableCategories'
import styles from './HidesEditor.module.css'
import editor from '../ItemEditorPage.module.css'

type Props = {
  contents: Record<string, unknown>
  category: string | null
  hides: string[]
  disabled?: boolean
  onChange: (hides: string[]) => void
  testId?: string
}

/** Which base body parts and wearable slots the wearable hides on the avatar (legacy Overrides section). */
export function HidesEditor({ contents, category, hides, disabled = false, onChange, testId = 'hides-editor' }: Props) {
  const { t } = useTranslation()
  const bodyParts = useMemo(() => getHideableBodyPartCategories(contents), [contents])
  const wearableCategories = useMemo(
    () => getHideableWearableCategories(contents, category, hides),
    [contents, category, hides]
  )
  const bodyPartOptions = useMemo(
    () => bodyParts.map(value => ({ value, label: t(`item_editor.body_part.${value}`) })),
    [bodyParts, t]
  )
  const categoryOptions = useMemo(
    () => wearableCategories.map(value => ({ value, label: t(`collection_detail_page.category.${value}`) })),
    [wearableCategories, t]
  )
  const hiddenBodyParts = useMemo(() => hides.filter(value => bodyParts.includes(value)), [hides, bodyParts])
  const hiddenCategories = useMemo(() => hides.filter(value => !bodyParts.includes(value)), [hides, bodyParts])

  // Each select owns its half of the list; the other half is carried over untouched.
  const setBodyParts = (values: string[]) => onChange([...values, ...hiddenCategories])
  const setCategories = (values: string[]) => onChange([...hiddenBodyParts, ...values])

  // Read-only (curators, locked items) lists every hidden slot as a pill: a disabled select truncates long lists.
  function renderReadOnly(label: (value: string) => string, values: string[], fieldTestId: string) {
    if (values.length === 0)
      return (
        <span className={styles.empty} data-testid={`${fieldTestId}-empty`}>
          {t('item_editor.overrides.none_hidden')}
        </span>
      )
    return (
      <div className={editor.chips} data-testid={fieldTestId}>
        {values.map(value => (
          <span className={editor.tagChip} key={value} data-readonly data-testid={`${fieldTestId}-pill`}>
            {label(value)}
          </span>
        ))}
      </div>
    )
  }

  return (
    <>
      <div className={styles.group}>
        <div className={styles.label}>
          {t('item_editor.overrides.body_parts')}
          <InfoTooltip content={t('item_editor.overrides.body_parts_hint')} testId={`${testId}-body-parts-hint`} />
        </div>
        {disabled && bodyPartOptions.length > 0 ? (
          renderReadOnly(value => t(`item_editor.body_part.${value}`), hiddenBodyParts, `${testId}-body-parts`)
        ) : bodyPartOptions.length === 0 ? (
          <span className={styles.empty}>{t('item_editor.overrides.none_available')}</span>
        ) : (
          <MultiSelect
            values={hiddenBodyParts}
            options={bodyPartOptions}
            placeholder={t('item_editor.overrides.none_selected')}
            disabled={disabled}
            tone="dark"
            ariaLabel={t('item_editor.overrides.body_parts')}
            testId={`${testId}-body-parts`}
            onChange={setBodyParts}
          />
        )}
      </div>
      <div className={styles.group}>
        <div className={styles.label}>
          {t('item_editor.overrides.categories')}
          <InfoTooltip content={t('item_editor.overrides.categories_hint')} testId={`${testId}-categories-hint`} />
        </div>
        {disabled && categoryOptions.length > 0 ? (
          renderReadOnly(
            value => t(`collection_detail_page.category.${value}`),
            hiddenCategories,
            `${testId}-categories`
          )
        ) : categoryOptions.length === 0 ? (
          <span className={styles.empty}>{t('item_editor.overrides.none_available')}</span>
        ) : (
          <MultiSelect
            values={hiddenCategories}
            options={categoryOptions}
            placeholder={t('item_editor.overrides.none_selected')}
            disabled={disabled}
            tone="dark"
            ariaLabel={t('item_editor.overrides.categories')}
            testId={`${testId}-categories`}
            onChange={setCategories}
          />
        )}
      </div>
    </>
  )
}
