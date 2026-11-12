import { forwardRef, useId, useState, type ReactNode } from 'react'
import {
  GeneralClose,
  GeneralDiscover,
  GeneralDownArrow,
  GeneralFavorites,
  GeneralFilters,
  GeneralSearch,
  GeneralSecondaryBarMyPlaces
} from '../../../foundations/icons'
import { Dialog } from './Dialog'
import styles from './ExploreBand.module.css'

type ExploreSection = 'all' | 'favourites' | 'my'
type CategoryOption = { value: string; label: string }

type ExploreBandLabels = {
  sections: Record<ExploreSection, string>
  sectionsGroup: string
  search: string
  searchPlaceholder: string
  category: string
  filters: string
  closeFilters: string
  removeFilter: (category: string) => string
}

type ExploreBandProps = {
  section: ExploreSection
  onSection: (section: ExploreSection) => void
  search: string
  onSearch: (value: string) => void
  category: string
  categories: readonly CategoryOption[]
  onCategory: (value: string) => void
  /** The value meaning "no category", which shows no chip. */
  allCategory: string
  labels: ExploreBandLabels
  children: ReactNode
}

const SECTION_ICONS = {
  all: GeneralDiscover,
  favourites: GeneralFavorites,
  my: GeneralSecondaryBarMyPlaces
} as const

/** TODO(figma-ds): a native select in the toolbar's dark pill, until the design system ships a Select. */
function CategorySelect({
  value,
  options,
  label,
  onChange,
  className
}: {
  value: string
  options: readonly CategoryOption[]
  label: string
  onChange: (value: string) => void
  className?: string
}) {
  return (
    <span className={`${styles.selectWrap} ${className ?? ''}`}>
      <select
        className={styles.select}
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      <GeneralDownArrow className={styles.selectIcon} size={20} aria-hidden="true" />
    </span>
  )
}

/**
 * Source's Explore band: the full-bleed darker band with the section pills, search and category
 * filter (a drawer on phones), above whatever the section shows.
 */
const ExploreBand = forwardRef<HTMLDivElement, ExploreBandProps>(function ExploreBand(
  {
    section,
    onSection,
    search,
    onSearch,
    category,
    categories,
    onCategory,
    allCategory,
    labels,
    children
  },
  ref
) {
  const searchId = useId()
  const [filtersOpen, setFiltersOpen] = useState(false)
  const filtering = section === 'all'
  const categoryLabel = categories.find((option) => option.value === category)?.label ?? category

  return (
    <div ref={ref} className={styles.band}>
      <div className={styles.content}>
        <div className={styles.toolbar}>
          <div className={styles.tabs} role="group" aria-label={labels.sectionsGroup}>
            {(Object.keys(SECTION_ICONS) as ExploreSection[]).map((id) => {
              const Icon = SECTION_ICONS[id]
              return (
                <button
                  key={id}
                  type="button"
                  className={id === section ? `${styles.tab} ${styles.tabActive}` : styles.tab}
                  aria-pressed={id === section}
                  onClick={() => onSection(id)}
                >
                  <Icon className={styles.tabIcon} aria-hidden="true" />
                  {labels.sections[id]}
                </button>
              )
            })}
          </div>
          {filtering ? (
            <div className={styles.controls}>
              <div className={styles.search}>
                <label htmlFor={searchId} className={styles.visuallyHidden}>
                  {labels.search}
                </label>
                <GeneralSearch className={styles.searchIcon} size={20} aria-hidden="true" />
                <input
                  id={searchId}
                  className={styles.searchInput}
                  type="search"
                  autoComplete="off"
                  placeholder={labels.searchPlaceholder}
                  value={search}
                  onChange={(event) => onSearch(event.target.value)}
                />
              </div>
              <CategorySelect
                className={styles.desktopOnly}
                value={category}
                options={categories}
                label={labels.category}
                onChange={onCategory}
              />
              <button
                type="button"
                className={styles.filterButton}
                aria-label={labels.filters}
                aria-haspopup="dialog"
                onClick={() => setFiltersOpen(true)}
              >
                <GeneralFilters size={24} aria-hidden="true" />
              </button>
            </div>
          ) : null}
        </div>
        {filtering && category !== allCategory ? (
          <div className={styles.chips}>
            <button
              type="button"
              className={styles.chip}
              aria-label={labels.removeFilter(categoryLabel)}
              onClick={() => onCategory(allCategory)}
            >
              {categoryLabel}
              <GeneralClose size={16} aria-hidden="true" />
            </button>
          </div>
        ) : null}
        {children}
      </div>
      {filtersOpen && filtering ? (
        <Dialog label={labels.filters} placement="drawer" onClose={() => setFiltersOpen(false)}>
          <div className={styles.drawerHeader}>
            <h2 className={styles.drawerTitle}>{labels.filters}</h2>
            <button
              type="button"
              className={styles.drawerClose}
              aria-label={labels.closeFilters}
              onClick={() => setFiltersOpen(false)}
            >
              <GeneralClose size={24} aria-hidden="true" />
            </button>
          </div>
          <div className={styles.drawerSection}>
            <span className={styles.drawerLabel} aria-hidden="true">
              {labels.category}
            </span>
            <CategorySelect
              value={category}
              options={categories}
              label={labels.category}
              onChange={onCategory}
            />
          </div>
        </Dialog>
      ) : null}
    </div>
  )
})

export { ExploreBand }
export type { CategoryOption, ExploreBandLabels, ExploreBandProps, ExploreSection }
