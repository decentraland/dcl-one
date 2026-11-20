import { useId, type ReactNode } from 'react'
import { Button, PlaceCard, Spinner, Tabs, TabsBar, type PlaceCardProps } from '../../../index'
import {
  GeneralFavorites,
  GeneralMapPin,
  GeneralPublic,
  GeneralVerified
} from '../../../foundations/icons'

import { EmptyMessage } from './EmptyMessage'
import styles from './ProfilePlaces.module.css'

type PlacesView = 'places' | 'favorites'

export const OWNER_EMPTY_TITLE = "Nothing here yet, but that's easy to change"

export type PlacesPanelState =
  | { kind: 'loading' }
  /** A read failed: say so, with the way to try again. Never shown as an empty list. */
  | { kind: 'error'; message: string; action: ReactNode }
  /** Nothing to list: the empty message, announced by its title. */
  | { kind: 'empty'; title: string; children: ReactNode }
  | { kind: 'empty-note'; message: string }
  | { kind: 'list'; count: string; cards: readonly (PlaceCardProps & { id: string })[] }

/** The owner's empty My Places: someone without a NAME needs one, someone with one needs to publish. */
export function PlacesOwnerEmpty({
  hasClaimedName,
  manageWorldHref,
  getNameHref
}: {
  hasClaimedName: boolean
  manageWorldHref: string
  getNameHref: string
}) {
  return hasClaimedName ? (
    <EmptyMessage
      icon={<GeneralMapPin size={56} />}
      title={OWNER_EMPTY_TITLE}
      subtitle="My Places shows the spaces you own or manage in Decentraland. You already have a NAME: publish a World on it and it will appear here."
      action={
        <Button href={manageWorldHref} size="md" leadingIcon={<GeneralPublic size={20} />}>
          Manage World
        </Button>
      }
    />
  ) : (
    <EmptyMessage
      icon={<GeneralMapPin size={56} />}
      title={OWNER_EMPTY_TITLE}
      subtitle="My Places shows the spaces you own or manage in Decentraland. The easiest way to start is with a NAME, which unlocks your own World."
      action={
        <Button href={getNameHref} size="md" leadingIcon={<GeneralVerified size={20} />}>
          Get a name
        </Button>
      }
    />
  )
}

/** One live region, always mounted, so a screen reader hears how loading ended. */
function StatusLine({ state }: { state: PlacesPanelState }) {
  return (
    <p className={state.kind === 'list' ? styles.count : styles.status} role="status">
      {state.kind === 'loading' ? (
        <>
          <Spinner variant="light" className={styles.spinner} aria-hidden="true" />
          <span className={styles.visuallyHidden}>Loading places</span>
        </>
      ) : state.kind === 'error' ? (
        state.message
      ) : state.kind === 'empty-note' ? (
        <span className={styles.note}>{state.message}</span>
      ) : state.kind === 'list' ? (
        state.count
      ) : (
        // The empty message below shows this title; the region only announces it.
        <span className={styles.visuallyHidden}>{state.title}</span>
      )}
    </p>
  )
}

export function ProfilePlaces({
  own,
  view,
  onView,
  state
}: {
  own: boolean
  view: PlacesView
  onView: (view: PlacesView) => void
  state: PlacesPanelState
}) {
  const ids = useId()
  const tabId = (id: PlacesView) => `${ids}-${id}`
  const panelId = `${ids}-panel`
  const filters = own ? (
    <TabsBar label="Places shown" variant="pill" className={styles.filters}>
      <Tabs
        id={tabId('places')}
        variant="pill"
        selected={view === 'places'}
        aria-controls={panelId}
        onClick={() => onView('places')}
        label={
          <span className={styles.tabLabel}>
            <GeneralMapPin size={20} aria-hidden="true" />
            My Places
          </span>
        }
      />
      <Tabs
        id={tabId('favorites')}
        variant="pill"
        selected={view === 'favorites'}
        aria-controls={panelId}
        onClick={() => onView('favorites')}
        label={
          <span className={styles.tabLabel}>
            <GeneralFavorites size={20} aria-hidden="true" />
            Favourites
          </span>
        }
      />
    </TabsBar>
  ) : null

  const panel = own ? { role: 'tabpanel', id: panelId, 'aria-labelledby': tabId(view) } : {}

  return (
    <div className={styles.root}>
      {filters}
      <div {...panel}>
        <StatusLine state={state} />
        {state.kind === 'error' ? <div className={styles.errorAction}>{state.action}</div> : null}
        {state.kind === 'empty' ? state.children : null}
        {state.kind === 'list' ? (
          <ul className={styles.grid}>
            {state.cards.map(({ id, ...card }) => (
              <li key={id} className={styles.item}>
                <PlaceCard {...card} />
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </div>
  )
}
