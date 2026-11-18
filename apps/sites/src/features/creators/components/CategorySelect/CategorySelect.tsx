import { useMemo } from 'react'
import { CategoryIcon } from '~/components/ItemIcons'
import { Select } from '~/components/Select'
import { useTranslation } from '~/intl'

type Props = {
  value: string | null
  categories: string[]
  onChange: (category: string) => void
  disabled?: boolean
  tone?: 'default' | 'dark'
  testId?: string
}

/** Wearable/emote category picker; every option and the current value carry the category glyph. */
export function CategorySelect({
  value,
  categories,
  onChange,
  disabled = false,
  tone,
  testId = 'category-select'
}: Props) {
  const { t } = useTranslation()
  const options = useMemo(
    () =>
      categories.map(category => ({
        value: category,
        label: t(`collection_detail_page.category.${category}`),
        icon: <CategoryIcon category={category} />
      })),
    [categories, t]
  )
  return (
    <Select
      value={value}
      options={options}
      onChange={onChange}
      placeholder={t('add_items_modal.select_category')}
      disabled={disabled}
      tone={tone}
      testId={testId}
    />
  )
}
