// The Shop's server-state cache: one QueryClient for each account the mounted Shop serves, never shared between
// hosts or sessions. When the Shop's account is replaced, by another account or by signing out, the Shop remounts
// with a fresh client and the replaced one is emptied, so no account's purchases, balances or favourites reach the
// next one. The first account to arrive (the profile loads after entering the world) keeps the client and the open
// Shop: until then it held only public reads. Query keys still name the account, chain or endpoint they read.

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { profileKey } from '@hud/features/session/profileStore'

// The reference defaults (docs/web-app-standards.md, "QueryClient"), as /play's in-world Shop uses them.
const newClient = (): QueryClient => new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: 30_000 } } })

export function ShopQueries({ account, children }: { account: string | undefined; children: ReactNode }): React.JSX.Element {
  const current = account == null ? '' : profileKey(account)
  const [session, setSession] = useState(() => ({ account: current, client: newClient(), generation: 0 }))
  if (session.account !== current) {
    setSession(
      session.account === ''
        ? { ...session, account: current }
        : { account: current, client: newClient(), generation: session.generation + 1 }
    )
  }
  // Emptied once it is replaced, after the commit: never on mount, which StrictMode runs twice.
  const shown = useRef(session.client)
  useEffect(() => {
    if (shown.current === session.client) return
    shown.current.clear()
    shown.current = session.client
  }, [session.client])
  return (
    <QueryClientProvider key={session.generation} client={session.client}>
      {children}
    </QueryClientProvider>
  )
}
