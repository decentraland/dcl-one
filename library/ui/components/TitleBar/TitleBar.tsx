import type { AnchorHTMLAttributes, ComponentType, HTMLAttributes, ReactNode } from 'react'
import { assertNavigationEntries, isPlainLinkClick, linkHref } from '../../link.helpers'
import { hasRenderableContent } from '../../content.helpers'
import styles from './TitleBar.module.css'

export type TitleBarSection = Readonly<{
  id: string
  label: string
  icon: ReactNode
  /** A route destination (http(s), relative or fragment); the section then renders as a page link. */
  href?: string
}>
/** Render one anchor and forward native attributes, children and events. Call the supplied onClick before
 * router cancellation; an earlier cancellation deliberately suppresses onSelect. Consume prefetch in the adapter. */
export type TitleBarLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  href: string
  prefetch?: 'intent'
  'data-active'?: boolean
}
type SharedProps = Omit<
  HTMLAttributes<HTMLDivElement>,
  'children' | 'dangerouslySetInnerHTML' | 'role' | 'aria-label' | 'title' | 'onSelect'
> & {
  title?: string
  /** Semantic heading level; the visual title style is unchanged. */
  headingLevel?: 1 | 2 | 3 | 4 | 5 | 6
  /** Accessible name of the whole bar; defaults to "<title> controls", or the title alone for Settings. */
  label?: string
  /** Accessible name of the sections group, or of their navigation landmark when any section is a route. */
  sectionsLabel?: string
  /** Invalid entries (missing or repeated IDs, empty labels, unsafe destinations) throw; validate remote
   * configuration at the app boundary. */
  sections: readonly TitleBarSection[]
  /** The current view or destination, not an independent on/off toggle. */
  activeId: string
  /** Controlled view-selection callback, also called on plain route clicks before app routing. Required when any
   * section is a view button (no href), and rendering throws without it; optional for route-only sections. */
  onSelect?: (id: string) => void
  LinkComponent?: ComponentType<TitleBarLinkProps>
}
export type TitleBarProps = SharedProps &
  (
    | { variant: 'settings'; controls?: never }
    | {
        variant: 'backpack'
        controls: { filters: ReactNode; search: ReactNode }
      }
    | { variant: 'events'; controls: { date: ReactNode; create: ReactNode } }
    | {
        variant: 'places'
        controls: { filters: ReactNode; search: ReactNode; filterMenu?: ReactNode }
      }
    | {
        variant: 'shop'
        controls: { filters: ReactNode; sort: ReactNode; search: ReactNode; cart: ReactNode }
      }
  )
const titles = {
  backpack: 'Backpack',
  settings: 'Settings',
  events: 'Events',
  places: 'Places',
  shop: 'Shop'
} as const
const controlOrder = {
  backpack: ['filters', 'search'],
  settings: [],
  events: ['date', 'create'],
  places: ['filters', 'search'],
  shop: ['filters', 'sort', 'search', 'cart']
} as const satisfies {
  [V in TitleBarProps['variant']]: readonly (keyof NonNullable<
    Extract<TitleBarProps, { variant: V }>['controls']
  >)[]
}

export function TitleBar({
  variant,
  title,
  headingLevel = 2,
  label,
  sectionsLabel,
  sections,
  activeId,
  onSelect,
  LinkComponent,
  className,
  controls,
  ...rest
}: TitleBarProps) {
  if (![1, 2, 3, 4, 5, 6].includes(headingLevel))
    throw new Error('TitleBar heading level must be an integer from 1 to 6')
  if (!Object.hasOwn(titles, variant)) throw new Error('Unsupported TitleBar variant')
  assertNavigationEntries(
    sections,
    (href) => href === undefined || linkHref(href) !== undefined,
    {
      id: 'TitleBar section IDs must be unique non-empty strings',
      entry: 'TitleBar sections require a non-empty label and destination'
    }
  )
  const routed = sections.some((section) => section.href !== undefined)
  if (
    (onSelect !== undefined || sections.some((section) => section.href === undefined)) &&
    typeof onSelect !== 'function'
  )
    throw new Error('TitleBar requires a controlled section callback')
  const Sections = routed ? 'nav' : 'div'
  const fields = (controls ?? {}) as Record<string, ReactNode>
  for (const field of controlOrder[variant])
    if (!Object.hasOwn(fields, field)) throw new Error('Missing TitleBar control: ' + field)
  const heading = title?.trim() ? title : titles[variant]
  const Heading = `h${headingLevel}` as 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6'
  return (
    <div
      {...rest}
      dangerouslySetInnerHTML={undefined}
      role="group"
      aria-label={label?.trim() || (variant === 'settings' ? heading : heading + ' controls')}
      data-variant={variant}
      className={[styles.root, className].filter(Boolean).join(' ')}
    >
      <div className={styles.primary} data-region="primary">
        <Heading className={styles.title}>{heading}</Heading>
        <Sections
          className={styles.sections}
          role={routed ? undefined : 'group'}
          aria-label={sectionsLabel?.trim() || heading + ' sections'}
        >
          {sections.map((section) => {
            const content = (
              <>
                <span className={styles.icon} aria-hidden="true">
                  {section.icon}
                </span>
                <span>{section.label}</span>
              </>
            )
            const active = section.id === activeId
            if (section.href !== undefined) {
              const props: TitleBarLinkProps = {
                href: section.href,
                className: styles.section,
                'data-active': active || undefined,
                'aria-current': active ? 'page' : undefined,
                onClick: (event) => {
                  if (isPlainLinkClick(event)) onSelect?.(section.id)
                }
              }
              return LinkComponent ? (
                <LinkComponent key={section.id} {...props} prefetch="intent">
                  {content}
                </LinkComponent>
              ) : (
                <a key={section.id} {...props}>
                  {content}
                </a>
              )
            }
            return (
              <button
                key={section.id}
                type="button"
                className={styles.section}
                aria-current={active ? 'true' : undefined}
                data-active={active || undefined}
                onClick={(event) => {
                  if (!event.defaultPrevented) onSelect?.(section.id)
                }}
              >
                {content}
              </button>
            )
          })}
        </Sections>
      </div>
      {variant !== 'settings' ? (
        <div className={styles.controls} data-region="controls">
          {controlOrder[variant].map((field) => (
            <div key={field} className={styles.slot} data-control={field}>
              {fields[field]}
              {field === 'filters' &&
              variant === 'places' &&
              hasRenderableContent(fields.filterMenu) ? (
                <div className={styles.filterMenu} data-control="filterMenu">
                  {fields.filterMenu}
                </div>
              ) : null}
            </div>
          ))}
        </div>
      ) : null}
    </div>
  )
}
