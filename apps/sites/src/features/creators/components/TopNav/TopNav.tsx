import { lazy, Suspense } from 'react'
import type { NavbarProps } from 'decentraland-ui2'
import * as S from './TopNav.styles'

// The global DCL top navbar pulls in all of decentraland-ui2 (MUI + emotion) — by far the biggest
// dependency. Lazy-load it so it stays out of the initial bundle: the app's own sub-nav + content
// paint immediately and this hydrates a beat later. A fixed-height placeholder holds its space so
// there's no layout shift. The `NavbarProps` import is type-only, so this file doesn't pull ui2 into
// the entry chunk.
const Navbar = lazy(() => import('decentraland-ui2/dist/components/Navbar').then(m => ({ default: m.Navbar })))

const TopNav = (props: NavbarProps) => {
  return (
    <Suspense fallback={<S.Skeleton aria-hidden />}>
      <S.NavbarViolet>
        <Navbar {...props} />
      </S.NavbarViolet>
    </Suspense>
  )
}

export { TopNav }
