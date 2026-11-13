import { forwardRef, useId, useState, type ReactNode, type SVGProps } from 'react'
import { GeneralClose } from '../../../foundations/icons'
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

/* The reference's tab glyphs (sites ToolbarIcons.tsx), inline with their exact path data: the library's drawings
   of the compass and heart round the coordinates, and its My Places glyph sits on a 24x25 box with an intrinsic
   size, measuring 19px wide at 19px tall where the reference's 20.18x24.94 box measures 15.36px. */
function BrowseGlyph(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" focusable="false" {...props}>
      <circle cx="11.6608" cy="12" r="8.72517" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
      <path
        d="M9.87193 10.2115L16.0176 7.6428L13.4489 13.7884L7.3032 16.3572L9.87193 10.2115Z"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="1.17455" cy="1.17455" r="1.17455" transform="matrix(-0.707107 0.707107 0.707107 0.707107 11.6606 10.3391)" fill="currentColor" />
    </svg>
  )
}

function FavoriteGlyph(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" focusable="false" {...props}>
      <path
        d="M16.104 3.84087C15.3297 3.83664 14.5635 4.00156 13.8559 4.32469C13.5105 4.48244 13.183 4.67613 12.8782 4.90211C12.5586 5.13904 12.264 5.41146 12 5.71517C11.736 5.41146 11.4414 5.13904 11.1218 4.90211C10.817 4.67613 10.4895 4.48244 10.1441 4.32469C9.43654 4.00156 8.67025 3.83664 7.896 3.84087C7.24163 3.82841 6.59141 3.95011 5.98334 4.19885C5.37527 4.44759 4.82155 4.81838 4.35453 5.28955C3.88752 5.76071 3.51659 6.32281 3.26341 6.94298C3.01023 7.56315 2.87989 8.22896 2.88 8.90149C2.88 12.9999 6.75777 16.258 9.97522 18.9613L9.97555 18.9616C10.2991 19.2335 10.616 19.4998 10.9217 19.7607C11.5428 20.2911 12.4594 20.2936 13.0821 19.7652C13.3625 19.5272 13.6523 19.2845 13.948 19.0369C17.183 16.3277 21.12 13.0306 21.12 8.90149C21.1201 8.22896 20.9898 7.56315 20.7366 6.94298C20.4834 6.32281 20.1125 5.76071 19.6455 5.28955C19.1785 4.81838 18.6247 4.44759 18.0167 4.19885C17.4086 3.95011 16.7584 3.82841 16.104 3.84087Z"
        stroke="currentColor"
        strokeWidth="2"
      />
    </svg>
  )
}

function MyPlacesGlyph(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 20.1813 24.9399" fill="none" focusable="false" {...props}>
      <path
        d="M1.00001 16.8939V21.0136C0.999902 21.106 1.01726 21.1974 1.0511 21.2827C1.08494 21.368 1.13458 21.4455 1.1972 21.5108C1.25982 21.5761 1.33419 21.6279 1.41604 21.6633C1.4979 21.6986 1.58564 21.7168 1.67426 21.7168H4.0511C4.18085 21.7169 4.30954 21.7411 4.43107 21.7884L9.69005 23.8348C9.89173 23.9133 10.1066 23.9485 10.3217 23.9382C10.5367 23.9279 10.7476 23.8724 10.9415 23.775L18.3203 20.0696C18.6739 19.8921 18.948 19.5791 19.0852 19.196C19.2225 18.8129 19.2123 18.3894 19.0568 18.014V18.014C18.9082 17.6557 18.638 17.3671 18.2979 17.2035C17.9578 17.0398 17.5716 17.0127 17.2135 17.1272L10.9763 19.1256L8.46201 16.6093C8.19462 16.3397 7.83727 16.1888 7.46519 16.1882H1.67102C1.58248 16.1886 1.49488 16.2073 1.41324 16.243C1.3316 16.2787 1.25751 16.3308 1.1952 16.3963C1.13289 16.4618 1.08358 16.5395 1.05009 16.6249C1.0166 16.7103 0.999582 16.8017 1.00001 16.8939Z"
        stroke="currentColor"
        strokeWidth="2"
        strokeMiterlimit="10"
      />
      <path
        d="M7 19.0012L8.96228 20.7147C9.1934 20.9046 9.49055 21.0062 9.79607 20.9997C10.1016 20.9932 10.3937 20.8792 10.6157 20.6797C10.7366 20.5707 10.833 20.4395 10.899 20.2941C10.9651 20.1488 10.9994 19.9922 11 19.8338C11.0006 19.6755 10.9673 19.5186 10.9023 19.3728C10.8373 19.227 10.7418 19.0953 10.6217 18.9855L8.48204 17"
        stroke="currentColor"
        strokeWidth="2"
        strokeMiterlimit="10"
        strokeLinecap="round"
      />
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M13.8208 11.087C12.9939 12.336 12.0398 13.4892 11.5022 14.1127C10.7674 14.9651 9.4568 14.9651 8.72192 14.1127C8.18434 13.4892 7.23022 12.336 6.40337 11.087C5.9903 10.4629 5.59177 9.78934 5.29246 9.12724C5.00338 8.48776 4.75484 7.74777 4.75484 7.02205C4.75484 4.04652 7.13452 1.6 10.1121 1.6C13.0896 1.6 15.4693 4.04652 15.4693 7.02205C15.4693 7.74777 15.2208 8.48776 14.9317 9.12724C14.6324 9.78934 14.2339 10.4629 13.8208 11.087ZM10.5373 13.2809C10.3107 13.5438 9.91349 13.5438 9.68683 13.2809C8.64112 12.0679 6.02883 8.85443 6.02883 7.02205C6.02883 4.73113 7.85696 2.87398 10.1121 2.87398C12.3672 2.87398 14.1953 4.73113 14.1953 7.02205C14.1953 8.85443 11.583 12.0679 10.5373 13.2809Z"
        fill="currentColor"
      />
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M10.1113 8.48809C10.9085 8.48809 11.5549 7.83149 11.5549 7.02154C11.5549 6.21159 10.9085 5.55499 10.1113 5.55499C9.31396 5.55499 8.66762 6.21159 8.66762 7.02154C8.66762 7.83149 9.31396 8.48809 10.1113 8.48809Z"
        fill="currentColor"
      />
    </svg>
  )
}

const SECTION_ICONS = {
  all: BrowseGlyph,
  favourites: FavoriteGlyph,
  my: MyPlacesGlyph
} as const

/* The reference's filter button icon: MUI's TuneRounded. */
function TuneGlyph(props: SVGProps<SVGSVGElement>) {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor" focusable="false" {...props}>
      <path d="M3 18c0 .55.45 1 1 1h5v-2H4c-.55 0-1 .45-1 1M3 6c0 .55.45 1 1 1h9V5H4c-.55 0-1 .45-1 1m10 14v-1h7c.55 0 1-.45 1-1s-.45-1-1-1h-7v-1c0-.55-.45-1-1-1s-1 .45-1 1v4c0 .55.45 1 1 1s1-.45 1-1M7 10v1H4c-.55 0-1 .45-1 1s.45 1 1 1h3v1c0 .55.45 1 1 1s1-.45 1-1v-4c0-.55-.45-1-1-1s-1 .45-1 1m14 2c0-.55-.45-1-1-1h-9v2h9c.55 0 1-.45 1-1m-5-3c.55 0 1-.45 1-1V7h3c.55 0 1-.45 1-1s-.45-1-1-1h-3V4c0-.55-.45-1-1-1s-1 .45-1 1v4c0 .55.45 1 1 1" />
    </svg>
  )
}

/* The reference's magnifier and select arrow, inline so they measure as it does. */
function SearchGlyph({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
      <path
        d="M10.5781 4C14.2111 4.00002 17.1562 6.94512 17.1562 10.5781C17.1562 11.8863 16.7734 13.1049 16.115 14.1293C16.0912 14.1662 16.1018 14.2102 16.1328 14.2412L19.6084 17.7178C20.1304 18.2399 20.1305 19.0863 19.6084 19.6084C19.0862 20.1305 18.2399 20.1304 17.7178 19.6084L14.2412 16.1328C14.2102 16.1018 14.1662 16.0912 14.1293 16.115C13.1049 16.7734 11.8863 17.1562 10.5781 17.1562C6.94517 17.1562 4.00002 14.211 4 10.5781C4 6.94515 6.94515 4 10.5781 4ZM10.5781 6.28027C8.20458 6.28027 6.28028 8.20452 6.28027 10.5781C6.28027 12.9517 8.20451 14.876 10.5781 14.876C12.9516 14.8759 14.876 12.9517 14.876 10.5781C14.876 8.2046 12.9516 6.2803 10.5781 6.28027Z"
        fill="currentColor"
      />
    </svg>
  )
}

function ArrowDropDownGlyph({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden="true"
      focusable="false"
      data-testid="ArrowDropDownIcon"
    >
      <path d="M7 10l5 5 5-5z" />
    </svg>
  )
}

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
  const selected = options.find((option) => option.value === value) ?? options[0]
  return (
    <span className={`${styles.selectWrap} ${className ?? ''}`}>
      <span className={styles.selectLabel} aria-hidden="true">
        {selected?.label ?? value}
      </span>
      <ArrowDropDownGlyph className={styles.selectIcon} />
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
                <SearchGlyph className={styles.searchIcon} />
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
                <TuneGlyph aria-hidden="true" data-testid="TuneRoundedIcon" />
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
