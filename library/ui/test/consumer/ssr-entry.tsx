import { renderToString } from 'react-dom/server'
import { NavBar, type NavBarProps } from '../../components/NavBar/NavBar'

export function renderNavigationRoute(props: NavBarProps, onError: (error: unknown) => string) {
  try {
    return { status: 200, html: renderToString(<NavBar {...props} />) }
  } catch (error) {
    return { status: 500, html: onError(error) }
  }
}
