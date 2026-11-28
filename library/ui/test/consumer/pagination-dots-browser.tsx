import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { PaginationDots } from '@dcl/one-ui'
import '@dcl/one-ui/foundations.css'

function Row({ appearance }: { appearance: 'default' | 'plain' }) {
  const [page, setPage] = useState(1)
  return (
    <div id={appearance} style={{ padding: 24 }}>
      <PaginationDots
        appearance={appearance}
        count={4}
        current={page}
        onSelect={setPage}
        label={appearance}
      />
    </div>
  )
}

createRoot(document.getElementById('root')!).render(
  <main style={{ background: '#161518' }}>
    <Row appearance="default" />
    <Row appearance="plain" />
  </main>
)
