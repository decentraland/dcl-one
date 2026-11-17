import { createRoot } from 'react-dom/client'
import { Button, InputExplorerNames, InputWebCreatorTools } from '@dcl/one-ui'
function App() {
  return (
    <div style={{ padding: 16 }}>
      <div id="web">
        <InputWebCreatorTools
          label="Email address"
          labelHidden
          defaultValue="jane.doe@example.com"
          action={<Button size="md">Save</Button>}
        />
      </div>
      <div id="explorer">
        <InputExplorerNames
          label="Name"
          defaultValue="Jane"
          action={<Button size="md">Claim</Button>}
        />
      </div>
    </div>
  )
}
createRoot(document.getElementById('root')!).render(<App />)
