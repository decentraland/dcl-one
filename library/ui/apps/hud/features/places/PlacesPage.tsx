// React Places — a full-screen page inside MainMenuShell (mirrors CommunitiesPage): browse
// Decentraland Places / Worlds and teleport to one. The browser body (tabs, search, categories,
// grid) is shared with the post-jump-in picker via PlacesBrowser; here a card opens the place's
// detail, whose Jump In teleports (parcel) or visits (world) and closes the page.

import { MainMenuShell } from '../menu/MainMenuShell'
import { MenuBackdrop } from '../menu/MenuBackdrop'
import { PlacesBrowser } from './PlacesBrowser'
import { openPlaceDetail } from './PlaceDetail'
import type { DiscoverPlace } from '@explorer-core/features/places/placesApi'
import { travelToPlace } from './PlaceActions'
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

  // the startup lobby's pick closes every page itself: a toggle would reopen this one
  const jumpIn = (picked: DiscoverPlace): boolean => {
    const outcome = travelToPlace(picked, { teleportToPlace: onTeleport, changeRealm: onVisitWorld })
    if (outcome === 'refused') return false
    if (compact != null) compact.onTravel()
    else if (outcome === 'sent') places.close()
    return true
  }
  const open = (place: DiscoverPlace): void => openPlaceDetail({ place, onJumpIn: jumpIn })

  if (compact != null) return <PlacesBrowser account={account} columns={COMPACT_PLACES_COLUMNS} onPick={open} />

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
        <PlacesBrowser account={account} onPick={open} />
      </div>
    </MainMenuShell>
  )
}
