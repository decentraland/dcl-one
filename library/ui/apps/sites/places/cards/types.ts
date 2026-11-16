import type { MouseEvent } from 'react'

/**
 * The card contract of /places, modeled on `WhatsOnCardData` (home/WhatsOn.tsx). The shared
 * PlaceCard and EventCard from the design system replace the provisional cards behind these slots;
 * the swap touches only the slot files.
 */
type PlaceCardCreator = {
  name: string
  /** The creator's profile; without it the name is plain text. */
  href?: string
  avatarUrl?: string
  /** Shows the Decentraland logo as the avatar, as source does for the Foundation's scenes. */
  foundation?: boolean
}

type PlaceCardData = {
  id: string
  title: string
  image: string | null
  /** The place page; the whole card follows it. */
  href: string
  /** People in the scene right now. */
  users?: number
  /** Names the user count for assistive technology, e.g. "17 people in-world". */
  usersLabel?: string
  creator?: PlaceCardCreator
  /** The parcel ("-3,-2") or, for a World, its name. */
  coords?: string
  world?: boolean
  /** Curated by the team (places-api `highlighted`). */
  featured?: boolean
  /** An event is running there now; `liveEventName` titles the LIVE badge. */
  live?: boolean
  liveEventName?: string
  /** Where Jump In goes: the web explorer on desktop, the app on a phone. No link, no button. */
  jumpHref?: string
}

type PlaceCardLabels = { by: string; jumpIn: string; featured: string; live: string }

type PlaceCardHandlers = {
  /** The card's page link was followed; call `preventDefault()` to handle it in place. */
  onOpen?: (card: PlaceCardData, event: MouseEvent<HTMLAnchorElement>) => void
  /** Jump In was activated; call `preventDefault()` to launch another way. */
  onJumpIn?: (card: PlaceCardData, event: MouseEvent<HTMLAnchorElement>) => void
}

type PlaceCardSlotProps =
  | ({ skeleton?: false; card: PlaceCardData; labels: PlaceCardLabels } & PlaceCardHandlers)
  /** A loading placeholder with the card's footprint. */
  | { skeleton: true; card?: never; labels?: never; onOpen?: never; onJumpIn?: never }

export type {
  PlaceCardCreator,
  PlaceCardData,
  PlaceCardHandlers,
  PlaceCardLabels,
  PlaceCardSlotProps
}
