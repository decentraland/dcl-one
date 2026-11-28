import { createRoot } from 'react-dom/client'
import { InputWebCreatorTools, WebSelect, WebTextarea } from '@dcl/one-ui'
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
      {/* A labelled select and textarea beside the text field whose label they repeat. */}
      <div style={{ width: 320, display: 'grid', gap: 24 }}>
        <InputWebCreatorTools label="Name" data-case="labelled-input" />
        <WebSelect label="Reason" data-case="labelled-select" defaultValue="spam">
          <option value="spam">Spam</option>
        </WebSelect>
        <WebTextarea label="Details" data-case="labelled-textarea" rows={3} />
        {/* A label given as an element keeps that element's own line height. */}
        <WebTextarea
          label={<span style={{ fontSize: 12, lineHeight: 'normal' }}>Small</span>}
          data-case="node-label"
          rows={2}
        />
      </div>
    </main>
  )
}

createRoot(document.getElementById('root')!).render(<Fixture />)
