export type SitesNavigationId =
  'explore' | 'whatson' | 'social' | 'shop' | 'create' | 'learn' | 'events'
export type SitesNavigationLink = {
  id: SitesNavigationId
  label: string
  href: string
  caret?: boolean
  menu?: { label: string; href: string }[]
}
export const sitesNavigationOrder = ['whatson', 'shop', 'create', 'social', 'learn'] as const

export function sitesNavigationLinks(docsHref: string) {
  return {
    explore: { id: 'explore', label: 'Explore', href: '/' },
    whatson: {
      id: 'whatson',
      label: 'Play',
      href: '/play/',
      caret: true,
      menu: [
        { label: 'Jump in', href: '/play/' },
        { label: "What's on", href: '/whats-on' }
      ]
    },
    social: { id: 'social', label: 'Social', href: '/social/' },
    shop: { id: 'shop', label: 'Shop', href: '/shop' },
    create: { id: 'create', label: 'Create', href: '/create' },
    learn: {
      id: 'learn',
      label: 'Learn',
      href: '/blog',
      caret: true,
      menu: [
        { label: 'Docs', href: docsHref },
        { label: 'Blog', href: '/blog' }
      ]
    },
    events: { id: 'events', label: 'Events', href: '/whats-on' }
  } satisfies Record<SitesNavigationId, SitesNavigationLink>
}
