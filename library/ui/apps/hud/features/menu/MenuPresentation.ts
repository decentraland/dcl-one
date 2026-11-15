// How a menu page presents itself. The startup lobby has no world behind it and no menu bar: a page
// it opens is a card over the dimmed lobby with only a close button, and the lobby says so here, so
// each page's shell styles its own card.

import { createContext } from 'react'

export type MenuPresentation = 'page' | 'lobbyCard'

export const MenuPresentationContext = createContext<MenuPresentation>('page')
