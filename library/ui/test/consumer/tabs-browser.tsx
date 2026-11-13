import { createRoot } from 'react-dom/client'
import { Tabs } from '@dcl/one-ui'
import '../../foundations/foundations.css'
createRoot(document.getElementById('root')!).render(
  <main style={{ padding: 16 }}>
    <nav id="tabs" style={{ display: 'flex', gap: 16 }}>
      <Tabs
        href="#overview"
        label={new URLSearchParams(location.search).get('label') ?? 'Overview'}
      />
      <Tabs href="#selected" label="Selected" selected />
    </nav>
  </main>
)
