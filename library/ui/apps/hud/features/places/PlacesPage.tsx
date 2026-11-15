// React Places — a full-screen page inside MainMenuShell (mirrors CommunitiesPage): browse
// Decentraland Places / Worlds and teleport to one. The browser body (tabs, search, categories,
// grid) is shared with the post-jump-in picker via PlacesBrowser; here a card click teleports
// (parcel) or visits (world) and closes the page.

import { MainMenuShell } from '../menu/MainMenuShell'
import { MenuBackdrop } from '../menu/MenuBackdrop'
import { openDestinationCard, placeInfo } from './DestinationCard'
import { PlacesBrowser } from './PlacesBrowser'
import { placeTeleport, type DiscoverPlace } from '@explorer-core/features/places/placesApi'
import type { ProfileState, TravelOutcome } from '../session/useEngineSession'
import styles from './PlacesPage.module.css'

export interface PlacesState {
  open: boolean
  toggle: () => void
}

export function PlacesPage({
  places,
  account,
  profile,
  onNavigate,
  onTeleport,
  onVisitWorld
}: {
  places: PlacesState
  account?: string | null
  profile: ProfileState
  onNavigate: (page: string) => void
  onTeleport: (x: number, y: number) => TravelOutcome
  onVisitWorld: (realm: string) => TravelOutcome
}): React.JSX.Element | null {
  if (!places.open) return null

  const visit = (place: DiscoverPlace): void => {
    const t = placeTeleport(place)
    if (!t) return
    openDestinationCard(placeInfo(place), () => {
      const outcome = t.kind === 'world' ? onVisitWorld(t.realm) : onTeleport(t.x, t.y)
      if (outcome === 'sent') places.toggle()
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
