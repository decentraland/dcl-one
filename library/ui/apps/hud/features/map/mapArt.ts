// Map art: the map's textures (category icons and pins, markers, toolbar and panel icons), kept
// under assets/map and statically imported (hashed, base-aware; a deleted or renamed file fails
// the build).
import catAll from '../../assets/map/categories/all.png'
import catArt from '../../assets/map/categories/art.png'
import catBusiness from '../../assets/map/categories/business.png'
import catEducation from '../../assets/map/categories/education.png'
import catFashion from '../../assets/map/categories/fashion.png'
import catFavorites from '../../assets/map/categories/favorites.png'
import catGame from '../../assets/map/categories/game.png'
import catMusic from '../../assets/map/categories/music.png'
import catShop from '../../assets/map/categories/shop.png'
import catSocial from '../../assets/map/categories/social.png'
import catSports from '../../assets/map/categories/sports.png'
import pinArt from '../../assets/map/pins/art.png'
import pinBusiness from '../../assets/map/pins/business.png'
import pinEducation from '../../assets/map/pins/education.png'
import pinFashion from '../../assets/map/pins/fashion.png'
import pinFavorites from '../../assets/map/pins/favorites.png'
import pinGame from '../../assets/map/pins/game.png'
import pinMusic from '../../assets/map/pins/music.png'
import pinShop from '../../assets/map/pins/shop.png'
import pinSocial from '../../assets/map/pins/social.png'
import pinSports from '../../assets/map/pins/sports.png'
import oceanGround from '../../assets/map/ocean/ground.png'
import oceanHeight from '../../assets/map/ocean/height.png'
import oceanNoise from '../../assets/map/ocean/noise.png'
import markersCompass from '../../assets/map/markers/compass.png'
import markersDestination from '../../assets/map/markers/destination.png'
import markersDestinationEllipse from '../../assets/map/markers/destination-ellipse.png'
import markersFocused from '../../assets/map/markers/focused.png'
import markersGeneric from '../../assets/map/markers/generic.png'
import markersHome from '../../assets/map/markers/home.png'
import markersLive from '../../assets/map/markers/live.png'
import markersParcelSelector from '../../assets/map/markers/parcel-selector.png'
import markersPinOutline from '../../assets/map/markers/pin-outline.png'
import markersPoi from '../../assets/map/markers/poi.png'
import markersQuest from '../../assets/map/markers/quest.png'
import iconsCenterHome from '../../assets/map/icons/center-home.png'
import iconsCenterPlayer from '../../assets/map/icons/center-player.png'
import iconsLayerLive from '../../assets/map/icons/layer-live.png'
import iconsLayerPoi from '../../assets/map/icons/layer-poi.png'
import iconsLayers from '../../assets/map/icons/layers.png'
import iconsParcelsView from '../../assets/map/icons/parcels-view.png'
import iconsPlus from '../../assets/map/icons/plus.png'
import iconsSatelliteView from '../../assets/map/icons/satellite-view.png'
import panelArrowLeft from '../../assets/map/panel/arrow-left.png'
import panelArrowRight from '../../assets/map/panel/arrow-right.png'
import panelCloseThick from '../../assets/map/panel/close-thick.png'
import panelCloseThin from '../../assets/map/panel/close-thin.png'
import panelDefaultPlace from '../../assets/map/panel/default-place.png'
import panelDislikeOutline from '../../assets/map/panel/dislike-outline.png'
import panelEmptyEvents from '../../assets/map/panel/empty-events.png'
import panelEmptySearch from '../../assets/map/panel/empty-search.png'
import panelFavouriteBlocked from '../../assets/map/panel/favourite-blocked.png'
import panelFavouriteOutline from '../../assets/map/panel/favourite-outline.png'
import panelFavouriteSolid from '../../assets/map/panel/favourite-solid.png'
import panelHomeOutline from '../../assets/map/panel/home-outline.png'
import panelHomeSolid from '../../assets/map/panel/home-solid.png'
import panelJumpIn from '../../assets/map/panel/jump-in.png'
import panelLikeOutline from '../../assets/map/panel/like-outline.png'
import panelLikeSolid from '../../assets/map/panel/like-solid.png'
import panelLink from '../../assets/map/panel/link.png'
import panelLive from '../../assets/map/panel/live.png'
import panelNavigation from '../../assets/map/panel/navigation.png'
import panelParcels from '../../assets/map/panel/parcels.png'
import panelPinOutline from '../../assets/map/panel/pin-outline.png'
import panelPlayers from '../../assets/map/panel/players.png'
import panelReminderOff from '../../assets/map/panel/reminder-off.png'
import panelReminderOn from '../../assets/map/panel/reminder-on.png'
import panelSearch from '../../assets/map/panel/search.png'
import panelShare from '../../assets/map/panel/share.png'
import panelTwitter from '../../assets/map/panel/twitter.png'
import panelWarning from '../../assets/map/panel/warning.png'

export const CAT_ICONS: Record<string, string> = {
  all: catAll, art: catArt, business: catBusiness, education: catEducation, fashion: catFashion,
  favorites: catFavorites, game: catGame, music: catMusic, shop: catShop, social: catSocial,
  sports: catSports
}

export const CAT_PINS: Record<string, string> = {
  art: pinArt, business: pinBusiness, education: pinEducation, fashion: pinFashion,
  favorites: pinFavorites, game: pinGame, music: pinMusic, shop: pinShop, social: pinSocial,
  sports: pinSports
}

/** Pin sprite for a place, from the first of its categories we have art for. Places with only
 *  categories that have no pin (`poi`, `featured`, …) fall back to the generic "all" icon,
 *  matching how MapPage resolves an unknown category. */
export function pinForCategories(categories: string[]): string {
  for (const c of categories) {
    const pin = CAT_PINS[c]
    if (pin != null) return pin
  }
  return CAT_ICONS.all
}

export const OCEAN_ART = { ground: oceanGround, height: oceanHeight, noise: oceanNoise } as const

export const MARKER_ART = {
  compass: markersCompass,
  destination: markersDestination,
  destinationEllipse: markersDestinationEllipse,
  focused: markersFocused,
  generic: markersGeneric,
  home: markersHome,
  live: markersLive,
  parcelSelector: markersParcelSelector,
  pinOutline: markersPinOutline,
  poi: markersPoi,
  quest: markersQuest
} as const

export const MAP_ICONS = {
  centerHome: iconsCenterHome,
  centerPlayer: iconsCenterPlayer,
  layerLive: iconsLayerLive,
  layerPoi: iconsLayerPoi,
  layers: iconsLayers,
  parcelsView: iconsParcelsView,
  plus: iconsPlus,
  satelliteView: iconsSatelliteView
} as const

export const PANEL_ART = {
  arrowLeft: panelArrowLeft,
  arrowRight: panelArrowRight,
  closeThick: panelCloseThick,
  closeThin: panelCloseThin,
  defaultPlace: panelDefaultPlace,
  dislikeOutline: panelDislikeOutline,
  emptyEvents: panelEmptyEvents,
  emptySearch: panelEmptySearch,
  favouriteBlocked: panelFavouriteBlocked,
  favouriteOutline: panelFavouriteOutline,
  favouriteSolid: panelFavouriteSolid,
  homeOutline: panelHomeOutline,
  homeSolid: panelHomeSolid,
  jumpIn: panelJumpIn,
  likeOutline: panelLikeOutline,
  likeSolid: panelLikeSolid,
  link: panelLink,
  live: panelLive,
  navigation: panelNavigation,
  parcels: panelParcels,
  pinOutline: panelPinOutline,
  players: panelPlayers,
  reminderOff: panelReminderOff,
  reminderOn: panelReminderOn,
  search: panelSearch,
  share: panelShare,
  twitter: panelTwitter,
  warning: panelWarning
} as const
