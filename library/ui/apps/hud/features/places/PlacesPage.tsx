// React Places — a full-screen page inside MainMenuShell (mirrors CommunitiesPage): browse
// Decentraland Places / Worlds and teleport to one. The browser body (tabs, search, categories,
// grid) is shared with the post-jump-in picker via PlacesBrowser; here a card click teleports
// (parcel) or visits (world) and closes the page.

import { MainMenuShell } from '../menu/MainMenuShell'
import { MenuBackdrop } from '../menu/MenuBackdrop'
import { openDestinationCard, placeInfo } from './DestinationCard'
import { PlacesBrowser } from './PlacesBrowser'
import { openPlaceDetail } from './PlaceDetail'
import { placeTeleport, type DiscoverPlace } from '@explorer-core/features/places/placesApi'
import type { ProfileState, TravelOutcome } from '../session/useEngineSession'
import styles from './PlacesPage.module.css'

export interface PlacesState {
  open: boolean
  toggle: () => void
  close: () => void
}

export const COMPACT_PLACES_COLUMNS = 4

export function PlacesPage({
  places,
  account,
  profile,
  compact,
  onNavigate,
  onTeleport,
  onVisitWorld
}: {
  places: PlacesState
  account?: string | null
  profile: ProfileState
  /** Shown in the lobby's section modal; `onTravel` closes it once a jump goes ahead. */
  compact?: { onTravel: () => void }
  onNavigate: (page: string) => void
  onTeleport: (x: number, y: number) => TravelOutcome
  onVisitWorld: (realm: string) => TravelOutcome
}): React.JSX.Element | null {
  if (!places.open && compact == null) return null

  const travel = (place: DiscoverPlace): TravelOutcome => {
    const t = placeTeleport(place)
    if (!t) return 'refused'
    return t.kind === 'world' ? onVisitWorld(t.realm) : onTeleport(t.x, t.y)
  }

  if (compact != null) {
    const { onTravel } = compact
    const open = (place: DiscoverPlace): void =>
      openPlaceDetail({
        place,
        onJumpIn: (picked) => {
          if (travel(picked) === 'refused') return false
          onTravel()
          return true
        }
      })
    return <PlacesBrowser account={account} columns={COMPACT_PLACES_COLUMNS} onPick={open} />
  }

  // the startup lobby's pick closes every page itself: a toggle would reopen this one
  const visit = (place: DiscoverPlace): void => {
    if (placeTeleport(place) == null) return
    openDestinationCard(placeInfo(place), () => {
      if (travel(place) === 'sent') places.close()
    })
  }

  const p = profile.data
  return (
    <MainMenuShell
      active="places"
      profileName={p?.name}
      profilePicture={p?.picture}
      profileAddress={p?.address}
      profileClaimed={p?.hasClaimedName}
      onNavigate={onNavigate}
      onClose={places.toggle}
      transparentBody
    >
      <div className={styles.page}>
        <MenuBackdrop />
        <PlacesBrowser account={account} onPick={visit} />
      </div>
    </MainMenuShell>
  )
}
