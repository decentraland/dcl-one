import { createRoot } from 'react-dom/client'
import '../../foundations/foundations.css'
import { Hero } from '../../apps/sites/home/Hero'
import { ComeHangOut } from '../../apps/sites/home/ComeHangOut'

const android = new URLSearchParams(location.search).get('store') === 'google_play'
const store = {
  badge: android ? ('google_play' as const) : ('app_store' as const),
  label: android ? 'Get it on Google Play' : 'Download on the App Store',
  href: android
    ? 'https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer'
    : 'https://apps.apple.com/app/decentraland/id1644311303'
}
const actions = {
  primary: {
    label: 'Download for',
    os: 'windows' as const,
    osName: 'Windows',
    href: '/download_success?os=windows'
  }
}

createRoot(document.getElementById('root')!).render(
  <>
    <div id="hero">
      <Hero
        title="Home"
        actions={actions}
        mobile={{ title: 'Mobile', subtitle: 'Decentraland', store }}
      />
    </div>
    <div id="closing">
      <ComeHangOut title="Come Hang Out" actions={actions} store={store} />
    </div>
  </>
)
