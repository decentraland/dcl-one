import type { DecentralandNavigationArea } from '../../../components/NavBar/NavBar.navigation'

export type SitesNavId =
  | 'play'
  | 'marketplace'
  | 'docs'
  | 'legal'
  | 'create'
  | 'explore'
  | 'whatson'
  | 'shop'
  | 'learn'
  /** No section is active: decentraland.org marks none on its download pages (its nav links are absolute URLs). */
  | 'none'

export const SITES_NAV_AREAS: Record<SitesNavId, DecentralandNavigationArea | ''> = {
  play: '',
  marketplace: 'shop',
  docs: 'learn',
  legal: '',
  create: 'create',
  explore: 'whatson',
  whatson: 'whatson',
  shop: 'shop',
  learn: 'learn',
  none: ''
}
