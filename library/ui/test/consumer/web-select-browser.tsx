import { createRoot } from 'react-dom/client'
import { WebSelect } from '@dcl/one-ui'
import '../../foundations/foundations.css'

function Fixture() {
  return (
    <main
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'flex-start',
        gap: 24,
        padding: 24,
        background: 'var(--dcl-neutrals-shadow)',
        color: 'var(--dcl-neutrals-snow)'
      }}
    >
      {/* As wide as its longest option, as the places category and the rows-per-page select are. */}
      <div style={{ display: 'inline-flex' }}>
        <WebSelect aria-label="Category" data-case="intrinsic" defaultValue="any">
          <option value="any">Any Category</option>
          <option value="art">Art</option>
        </WebSelect>
      </div>
      <div style={{ display: 'inline-flex' }}>
        <WebSelect aria-label="Rows per page" data-case="digits" defaultValue="10">
          <option value="10">10</option>
          <option value="25">25</option>
        </WebSelect>
      </div>
      {/* Narrower than its text, which has to stop short of the arrow. */}
      <div style={{ width: 150 }}>
        <WebSelect aria-label="Network" data-case="narrow" defaultValue="long">
          <option value="long">A network with a very long name</option>
        </WebSelect>
      </div>
      <div style={{ width: 320 }}>
        <WebSelect aria-label="Reason" data-case="wide" defaultValue="spam">
          <option value="spam">Spam</option>
        </WebSelect>
      </div>
    </main>
  )
}

createRoot(document.getElementById('root')!).render(<Fixture />)
