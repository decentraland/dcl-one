import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import '../../foundations/foundations.css'
import { Button, NavBar } from '@dcl/one-ui'

function CompositionExample() {
  const [connections, setConnections] = useState(0)
  return (
    <>
      <NavBar
        sections={[{ label: 'Discover', href: '/discover' }]}
        account={{ address: '0x1234', name: 'Ada' }}
        balances={{ credits: 8, mana: 2 }}
        balanceAction={
          <Button id="balance-action" variant="secondary" size="sm">
            Buy credits
          </Button>
        }
        accountControl={
          <Button
            id="account-control"
            variant="secondary"
            size="sm"
            onClick={() => setConnections((value) => value + 1)}
          >
            Connect wallet
          </Button>
        }
        mobileSecondaryNavigation={
          <nav aria-label="Profile sections">
            <Button id="secondary-link" href="/profile/photos" variant="text" size="md">
              Photos
            </Button>
          </nav>
        }
      />
      <main>
        <p role="status">Connections: {connections}</p>
      </main>
    </>
  )
}

createRoot(document.getElementById('root')!).render(<CompositionExample />)
