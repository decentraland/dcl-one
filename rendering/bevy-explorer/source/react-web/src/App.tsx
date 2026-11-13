// react-web's HUD: the shared one (library/ui/apps/hud/App.tsx) with this host's drivers, which add
// the benchmark autopilot's sign-in, and its Shop page, the catalogue with a query client of its own.
import { HudApp, type HudShopProps } from '@hud/App'
import { ShopPage } from '@hud/features/shop/ShopPage'
import { AutopilotBridgeClient, AutopilotEngineDriver } from './engine/autopilot'
import { ShopQueries } from './features/shop/ShopQueries'

function Shop({ session, onNavigate }: HudShopProps): React.JSX.Element {
  return (
    <ShopQueries account={session.profile.data?.address}>
      <ShopPage shop={session.shop} profile={session.profile} onNavigate={onNavigate} />
    </ShopQueries>
  )
}

export function App(): React.JSX.Element {
  return (
    <HudApp
      createEngineDriver={(rpc) => new AutopilotEngineDriver(rpc)}
      createBridgeClient={() => new AutopilotBridgeClient()}
      ShopPage={Shop}
    />
  )
}
