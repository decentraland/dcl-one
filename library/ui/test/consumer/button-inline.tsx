import { createRoot } from 'react-dom/client'
import { Button } from '@dcl/one-ui'
import '../../foundations/foundations.css'

/** Running text in two type settings, each holding an inline Button between two plain words. */
function Fixture() {
  return (
    <main style={{ padding: 16, background: 'var(--dcl-neutrals-shadow)' }}>
      <p
        data-case="body"
        style={{ font: '400 14px/21px Inter, sans-serif', color: 'rgb(252, 252, 252)', margin: 0 }}
      >
        <span data-before>Wallet</span> <Button variant="inline">0x4b53...5ff9 (Ana)</Button>{' '}
        <span data-after>granted</span>
      </p>
      <p
        data-case="byline"
        style={{
          font: '500 15px/24px Inter, sans-serif',
          letterSpacing: '0.15px',
          textTransform: 'none',
          color: 'rgb(255, 45, 85)',
          margin: 0
        }}
      >
        <span data-before>By</span> <Button variant="inline">AudioHotLab</Button>{' '}
        <span data-after>here</span>
      </p>
    </main>
  )
}

createRoot(document.getElementById('root')!).render(<Fixture />)
