import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { Dropdown } from '../../components/Dropdown/Dropdown'
import { Modal } from '../../components/Modal/Modal'
import '../../foundations/foundations.css'

// `?hidden=1` hides the title; `?dropdown=1` puts the places category Dropdown in the drawer.
const query = new URLSearchParams(location.search)
const options = [
  { value: 'all', label: 'Any Category' },
  { value: 'music', label: 'Music' },
  { value: 'games', label: 'Games' }
]

document.body.style.margin = '0'

function Fixture() {
  const [open, setOpen] = useState(true)
  const [category, setCategory] = useState('all')
  return (
    <main style={{ minHeight: '200vh' }}>
      <p data-state>{open ? 'open' : 'closed'}</p>
      {open ? (
        <Modal
          layout="drawer"
          title="Filters"
          titleHidden={query.has('hidden')}
          closeLabel="Close filters"
          onClose={() => setOpen(false)}
        >
          {query.has('dropdown') ? (
            <Dropdown
              variant="dark"
              label="Category"
              labelHidden
              options={options}
              value={category}
              onChange={setCategory}
            />
          ) : (
            <div data-section style={{ height: 68 }} />
          )}
        </Modal>
      ) : null}
    </main>
  )
}

createRoot(document.getElementById('root')!).render(<Fixture />)
