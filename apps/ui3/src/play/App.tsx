// /play's HUD: the shared one (library/ui/apps/hud/App.tsx) with the shared drivers, the in-world
// Shop page (`?shop=old` keeps the catalogue while it is retired) and the HUD's one server-state cache.
import { lazy, Suspense, useState, type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { HudApp, type HudShopProps } from '@hud/App'
import { ShopPage } from '@hud/features/shop/ShopPage'
import { BridgeClient } from '@explorer-core/engine/bridge'
import { EngineDriver } from '@explorer-core/engine/EngineDriver'
import { IN_WORLD_SHOP } from './features/shop/entry'

// The in-world Shop, loaded only when it is first opened. lazy() keeps a failed load, so a crash
// swaps in a fresh one for the next open.
const loadInWorldShop = () => lazy(() => import('./features/shop/InWorldShop'))
let InWorldShop = /* @__PURE__ */ loadInWorldShop()

// The HUD's one server-state cache (docs/web-app-standards.md, "QueryClient"), with the reference defaults.
function Providers({ children }: { children: ReactNode }): React.JSX.Element {
  const [queryClient] = useState(() => new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: 30_000 } } }))
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
}

function Shop({ session, onNavigate, mock }: HudShopProps): React.JSX.Element | null {
  // mounted for good once the Shop is first opened (unless `?shop=old`)
  const [wanted, setWanted] = useState(false)
  if (IN_WORLD_SHOP && !wanted && session.shop.open) setWanted(true)
  if (!IN_WORLD_SHOP) return <ShopPage shop={session.shop} profile={session.profile} onNavigate={onNavigate} />
  if (!wanted) return null
  return (
    <Suspense fallback={null}>
      <InWorldShop
        shop={session.shop}
        profile={session.profile}
        onNavigate={onNavigate}
        setEngineViewport={session.setEngineViewport}
        equipped={session.backpack.equipped}
        preview={session.backpack.preview}
        focus={session.backpack.focus}
        stage={session.backpack.stage}
        emote={session.backpack.emote}
        rotate={session.backpack.rotate}
        zoom={session.backpack.zoom}
        noEngine={mock}
      />
    </Suspense>
  )
}

export function App(): React.JSX.Element {
  return (
    <HudApp
      createEngineDriver={(rpc) => new EngineDriver(rpc)}
      createBridgeClient={() => new BridgeClient()}
      ShopPage={Shop}
      onShopCrash={() => {
        if (IN_WORLD_SHOP) InWorldShop = loadInWorldShop()
      }}
      Providers={Providers}
    />
  )
}
