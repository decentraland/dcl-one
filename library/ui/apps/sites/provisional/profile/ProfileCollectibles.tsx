// TODO(figma-ds): replace with the Figma-generated profile collectibles panel from library/ui/components.
import type { ComponentType, ReactNode, Ref } from 'react'
import { GeneralName, GeneralRightArrow } from '../../../../foundations/icons'
import styles from './ProfileCollectibles.module.css'

type ProfileCollectiblesState =
  | { kind: 'loading'; label: string }
  /** A read failed: say so, with the way to try again. Never shown as an empty list. */
  | { kind: 'error'; message: string; action?: ReactNode }
  /** Nothing to list: an EmptyState, or a one-line note. */
  | { kind: 'empty'; label: string; children: ReactNode }
  /** `more` keeps Load more under a note when a filter matched nothing on the pages loaded so far. */
  | { kind: 'empty-note'; message: string; more?: ReactNode }
  | {
      kind: 'list'
      /** Announced when loading ends; shown above the list unless `showCount` is false. */
      count: string
      showCount?: boolean
      /** Cards in a grid, full-width rows (NAMEs), or the photo grid. */
      layout?: 'grid' | 'rows' | 'photos'
      children: ReactNode
      /** The list element, so the page can move focus to a row it loaded. */
      listRef?: Ref<HTMLUListElement>
      /** Under the list: Load more, its progress, or why the next page failed. */
      more?: ReactNode
    }

type ProfileCollectiblesProps = {
  /** The category chips; shown above every state. */
  filters?: ReactNode
  /** Beside the filters, e.g. a "View all" link. */
  aside?: ReactNode
  state: ProfileCollectiblesState
  /** Makes the panel focusable from script, so focus has somewhere to go when a control leaves. */
  panelRef?: Ref<HTMLDivElement>
}

/** A profile's Creations, Assets or Photos tab body: the filters, then loading, an error, an empty state or the list. */
function ProfileCollectibles({ filters, aside, state, panelRef }: ProfileCollectiblesProps) {
  const list = state.kind === 'list' ? state : null
  const countShown = list !== null && list.showCount !== false
  const more = state.kind === 'list' || state.kind === 'empty-note' ? state.more : undefined

  return (
    <div ref={panelRef} className={styles.root} tabIndex={panelRef ? -1 : undefined}>
      {filters || aside ? (
        <div className={styles.header}>
          {filters}
          {aside ? <div className={styles.aside}>{aside}</div> : null}
        </div>
      ) : null}
      {/* One live region, always mounted, so a screen reader hears how loading ended. */}
      <p className={countShown ? styles.count : styles.status} role="status">
        {state.kind === 'loading' ? (
          <>
            <span className={styles.spinner} aria-hidden="true" />
            <span className={styles.visuallyHidden}>{state.label}</span>
          </>
        ) : state.kind === 'error' ? (
          state.message
        ) : state.kind === 'empty-note' ? (
          <span className={styles.note}>{state.message}</span>
        ) : state.kind === 'list' ? (
          countShown ? (
            state.count
          ) : (
            <span className={styles.visuallyHidden}>{state.count}</span>
          )
        ) : (
          // The EmptyState below shows this title; the region only announces it.
          <span className={styles.visuallyHidden}>{state.label}</span>
        )}
      </p>
      {state.kind === 'error' && state.action ? (
        <div className={styles.errorAction}>{state.action}</div>
      ) : null}
      {state.kind === 'empty' ? state.children : null}
      {list ? (
        <ul ref={list.listRef} className={styles[list.layout ?? 'grid']}>
          {list.children}
        </ul>
      ) : null}
      {more ? <div className={styles.more}>{more}</div> : null}
    </div>
  )
}

/** One cell of the list; wraps a CatalogCard or a ProfileNameCard. */
function ProfileCollectiblesItem({ children }: { children: ReactNode }) {
  return <li className={styles.item}>{children}</li>
}

/** The spinner the list shows while its next page loads. */
function ProfileCollectiblesProgress({ label }: { label: string }) {
  return (
    <span className={styles.progress}>
      <span className={styles.spinnerSmall} aria-hidden="true" />
      <span className={styles.visuallyHidden}>{label}</span>
    </span>
  )
}

/** The app's router link, so following it is a client navigation; a plain anchor otherwise. */
type ProfileCollectiblesLinkProps = { to: string; className: string; children: ReactNode }

function AnchorLink({ to, ...props }: ProfileCollectiblesLinkProps) {
  return <a href={to} {...props} />
}

type ProfileCollectiblesViewAllProps = {
  href: string
  label: string
  LinkComponent?: ComponentType<ProfileCollectiblesLinkProps>
}

/** The header's link to the full list elsewhere, with its chevron. */
function ProfileCollectiblesViewAll({
  href,
  label,
  LinkComponent = AnchorLink
}: ProfileCollectiblesViewAllProps) {
  return (
    <LinkComponent to={href} className={styles.viewAll}>
      {label}
      <GeneralRightArrow size={16} aria-hidden="true" />
    </LinkComponent>
  )
}

type ProfileNameCardProps = {
  /** The NAME without its suffix. */
  name: string
  suffix?: string
  /** Its Edit and Transfer links. */
  actions?: ReactNode
}

/** A NAME as a row: the NAME tile, the name and its `.dcl.eth` suffix, and its actions. */
function ProfileNameCard({ name, suffix = '.dcl.eth', actions }: ProfileNameCardProps) {
  return (
    <div className={styles.name}>
      <span className={styles.nameTile} aria-hidden="true">
        <GeneralName size={26} />
      </span>
      <p className={styles.nameLabel}>
        {name}
        <span className={styles.nameSuffix}>{suffix}</span>
      </p>
      {actions ? <div className={styles.nameActions}>{actions}</div> : null}
    </div>
  )
}

export {
  ProfileCollectibles,
  ProfileCollectiblesItem,
  ProfileCollectiblesProgress,
  ProfileCollectiblesViewAll,
  ProfileNameCard
}
export type {
  ProfileCollectiblesLinkProps,
  ProfileCollectiblesProps,
  ProfileCollectiblesState,
  ProfileCollectiblesViewAllProps,
  ProfileNameCardProps
}
