import { createRoot } from 'react-dom/client'
import { useState } from 'react'
import { Tabs } from '@dcl/one-ui'
import '../../foundations/foundations.css'

const panels = [
  { id: 'underline', variant: 'underline', device: 'desktop' },
  { id: 'mobile', variant: 'underline', device: 'mobile' },
  { id: 'compact', variant: 'compact', device: 'desktop' },
  { id: 'pill', variant: 'pill', device: undefined }
] as const

function PanelTabs({ id, variant, device }: (typeof panels)[number]) {
  const [selected, setSelected] = useState(0)
  return (
    <div id={id} role="tablist" style={{ display: 'flex', gap: 16 }}>
      {['First', 'Second'].map((label, index) => (
        <Tabs
          key={label}
          label={label}
          variant={variant}
          device={device}
          selected={selected === index}
          onClick={() => setSelected(index)}
        />
      ))}
      <Tabs label="Disabled" variant={variant} device={device} disabled />
    </div>
  )
}

createRoot(document.getElementById('root')!).render(
  <main style={{ padding: 16 }}>
    <nav id="tabs" style={{ display: 'flex', gap: 16 }}>
      <Tabs
        mode="navigation"
        href="#overview"
        label={new URLSearchParams(location.search).get('label') ?? 'Overview'}
      />
      <Tabs mode="navigation" href="#selected" label="Selected" selected />
    </nav>
    {panels.map((panel) => (
      <PanelTabs key={panel.id} {...panel} />
    ))}
  </main>
)
