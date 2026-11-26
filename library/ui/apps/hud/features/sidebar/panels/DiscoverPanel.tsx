// DiscoverPanel — the panel the Discover button opens: the three pages that list what there is to
// do, each with the key that opens it. Events carries the count of live events and Communities the
// mark of a live stream in one of the player's communities. The bar places it and closes it; every
// action and every key comes in by props.

import { CountBadge, MenuList, MenuPanel, MenuRow } from '@dcl/one-ui'
import eventsIcon from '../../../assets/sidebar-rail/discover-events.svg?no-inline'
import placesIcon from '../../../assets/sidebar-rail/discover-places.svg?no-inline'
import communitiesIcon from '../../../assets/sidebar-rail/discover-communities.svg?no-inline'
import streamBadge from '../../../assets/voice/stream-badge.png'

export interface DiscoverPanelProps {
  /** The key bound to the Events page, drawn as "[X]" after the label. */
  eventsKey?: string
  /** The key bound to the Places page. */
  placesKey?: string
  /** The key bound to the Communities page. */
  communitiesKey?: string
  /** Events live right now; no count at 0. */
  liveEvents?: number
  /** A community the player belongs to is streaming. */
  communityStreamLive?: boolean
  onEvents: () => void
  onPlaces: () => void
  onCommunities: () => void
  /** Called after a row's action, so the bar closes the panel. */
  onClose: () => void
}

// Each picture is the design's icon centred in the row's 28px box.
const art = (src: string): React.JSX.Element => <img src={src} alt="" width={28} height={28} draggable={false} />

export function DiscoverPanel({
  eventsKey,
  placesKey,
  communitiesKey,
  liveEvents = 0,
  communityStreamLive = false,
  onEvents,
  onPlaces,
  onCommunities,
  onClose
}: DiscoverPanelProps): React.JSX.Element {
  const pick = (act: () => void) => (): void => {
    act()
    onClose()
  }
  return (
    <MenuPanel label="Discover" title="DISCOVER">
      <MenuList>
        <MenuRow
          icon={art(eventsIcon)}
          label="Events"
          hint={eventsKey}
          trailing={liveEvents > 0 ? <CountBadge count={liveEvents} size="sm" /> : undefined}
          onClick={pick(onEvents)}
        />
        <MenuRow icon={art(placesIcon)} label="Places" hint={placesKey} onClick={pick(onPlaces)} />
        {/* the row keeps the name "Communities" whether or not the mark is drawn */}
        <MenuRow
          icon={art(communitiesIcon)}
          label="Communities"
          aria-label="Communities"
          hint={communitiesKey}
          trailing={communityStreamLive ? <img src={streamBadge} alt="A community stream is live" width={16} height={16} draggable={false} /> : undefined}
          onClick={pick(onCommunities)}
        />
      </MenuList>
    </MenuPanel>
  )
}
