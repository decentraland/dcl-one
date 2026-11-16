import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { Tabs, TabsBar } from '../../index'
import '../../foundations/foundations.css'
function App() {
  const [selected, setSelected] = useState('first')
  return (
    <main>
      <TabsBar label="Legacy panels">
        <Tabs label="First" selected={selected === 'first'} onClick={() => setSelected('first')} />
        <Tabs
          label="Second"
          selected={selected === 'second'}
          onClick={() => setSelected('second')}
        />
      </TabsBar>
      <TabsBar label="Legacy pills" variant="pill">
        <Tabs label="Pill" variant="pill" selected />
      </TabsBar>
      <Tabs label="Anchor panel" href="#panel" selected />
    </main>
  )
}
createRoot(document.getElementById('root')!).render(<App />)
