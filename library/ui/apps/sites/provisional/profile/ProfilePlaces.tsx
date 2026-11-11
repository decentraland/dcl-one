// TODO(figma-ds): replace with the Figma-generated profile places panel from library/ui/components.
import type { ReactNode } from 'react'
import styles from './ProfilePlaces.module.css'

type ProfilePlacesState =
  | { kind: 'loading'; label: string }
  /** A read failed: say so, with the way to try again. Never shown as an empty list. */
  | { kind: 'error'; message: string; action?: ReactNode }
  /** Nothing to list: an EmptyState, or a one-line note for a visitor. */
  | { kind: 'empty'; label: string; children: ReactNode }
  | { kind: 'empty-note'; message: string }
  | { kind: 'list'; count: string; children: ReactNode }

type ProfilePlacesProps = {
  /** The owner's view switch; shown above every state. */
  filters?: ReactNode
  state: ProfilePlacesState
}

/** The jump-in mark the source puts after its "send me in-world" calls to action. */
function JumpInBadgeIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect
        x="0.75"
        y="0.75"
        width="22.5"
        height="22.5"
        rx="7.25"
        stroke="currentColor"
        strokeOpacity="0.7"
        strokeWidth="1.5"
      />
      <path
        d="M18.7111 11.065L14.034 6.39027C13.2002 5.55695 11.7971 6.14637 11.7971 7.32523V8.86994C11.7564 8.86994 11.7361 8.86994 11.6954 8.86994H7.25895C6.50654 8.86994 5.89648 9.45936 5.89648 10.2114V13.7683C5.89648 14.5203 6.50654 15.1301 7.25895 15.1301H11.6751C11.7158 15.1301 11.7361 15.1301 11.7768 15.1301V16.6748C11.7768 17.8536 13.2002 18.4431 14.0137 17.6097L18.6908 12.935C19.2195 12.4065 19.2195 11.5732 18.7111 11.065Z"
        fill="currentColor"
      />
    </svg>
  )
}

/** A profile's Places tab body: the filters, then loading, an error, an empty state or the grid. */
function ProfilePlaces({ filters, state }: ProfilePlacesProps) {
  return (
    <div className={styles.root}>
      {filters}
      {/* One live region, always mounted, so a screen reader hears how loading ended. */}
      <p className={state.kind === 'list' ? styles.count : styles.status} role="status">
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
          state.count
        ) : (
          // The EmptyState below shows this title; the region only announces it.
          <span className={styles.visuallyHidden}>{state.label}</span>
        )}
      </p>
      {state.kind === 'error' && state.action ? (
        <div className={styles.errorAction}>{state.action}</div>
      ) : null}
      {state.kind === 'empty' ? state.children : null}
      {state.kind === 'list' ? <ul className={styles.grid}>{state.children}</ul> : null}
    </div>
  )
}

/** One cell of the grid; wraps a SceneCard. */
function ProfilePlacesItem({ children }: { children: ReactNode }) {
  return <li className={styles.item}>{children}</li>
}

export { JumpInBadgeIcon, ProfilePlaces, ProfilePlacesItem }
export type { ProfilePlacesProps, ProfilePlacesState }
