import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { PaginationDots } from '@dcl/one-ui'
import '@dcl/one-ui/foundations.css'

function Row({
  id,
  appearance,
  spacing
}: {
  id: string
  appearance: 'default' | 'plain'
  spacing?: 'compact' | 'wide'
}) {
  const [page, setPage] = useState(1)
  return (
    <div id={id} style={{ padding: 24 }}>
      <PaginationDots
        appearance={appearance}
        spacing={spacing}
        count={4}
        current={page}
        onSelect={setPage}
        label={id}
      />
    </div>
  )
}

createRoot(document.getElementById('root')!).render(
  <main style={{ background: '#161518' }}>
    <Row id="default" appearance="default" />
    <Row id="plain" appearance="plain" />
    <Row id="wide" appearance="plain" spacing="wide" />
  </main>
)
