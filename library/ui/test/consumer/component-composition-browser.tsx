import { createRoot } from 'react-dom/client'
import { Button, Tabs, InputExplorerNames } from '@dcl/one-ui'
import { InputField } from '../../components/InputField/InputField'
import '../../foundations/foundations.css'
createRoot(document.getElementById('root')!).render(
  <main style={{ padding: 32, background: 'var(--dcl-bg-page)', color: 'var(--dcl-text-primary)' }}>
    <nav id="mixed-tabs">
      <Tabs href="#selected" label="Overview" selected />
      <Tabs href="#iap" label="IAP" />
    </nav>
    <InputExplorerNames id="explorer-invalid" label="Explorer error" error="Invalid name" />
    <InputField surface="web" id="web-invalid" label="Internal web error" error="Invalid name" />
    <Button id="outside" size="sm">
      Outside
    </Button>
  </main>
)
