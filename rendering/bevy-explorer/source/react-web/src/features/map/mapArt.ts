// Map art — the actual Unity (unity-explorer) MapRenderer textures (AllIcn/ArtIcn/…,
// PinArt/PinGames/…), copied to src/assets/map under our category keys and statically
// imported (hashed, base-aware, a deleted/renamed file fails the BUILD).
import catAll from '@hud/assets/map/categories/all.png'
import catArt from '@hud/assets/map/categories/art.png'
import catBusiness from '@hud/assets/map/categories/business.png'
import catEducation from '@hud/assets/map/categories/education.png'
import catFashion from '@hud/assets/map/categories/fashion.png'
import catFavorites from '@hud/assets/map/categories/favorites.png'
import catGame from '@hud/assets/map/categories/game.png'
import catMusic from '@hud/assets/map/categories/music.png'
import catShop from '@hud/assets/map/categories/shop.png'
import catSocial from '@hud/assets/map/categories/social.png'
import catSports from '@hud/assets/map/categories/sports.png'
import pinArt from '@hud/assets/map/pins/art.png'
import pinBusiness from '@hud/assets/map/pins/business.png'
import pinEducation from '@hud/assets/map/pins/education.png'
import pinFashion from '@hud/assets/map/pins/fashion.png'
import pinFavorites from '@hud/assets/map/pins/favorites.png'
import pinGame from '@hud/assets/map/pins/game.png'
import pinMusic from '@hud/assets/map/pins/music.png'
import pinShop from '@hud/assets/map/pins/shop.png'
import pinSocial from '@hud/assets/map/pins/social.png'
import pinSports from '@hud/assets/map/pins/sports.png'
import worldPng from '@hud/assets/map/world.png'

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

export const WORLD_ICON = worldPng

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
