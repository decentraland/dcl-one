import { createRoot } from 'react-dom/client'
import { Modal } from '../../components/Modal/Modal'
import '../../foundations/foundations.css'

document.body.style.margin = '0'

createRoot(document.getElementById('root')!).render(
  <main style={{ minHeight: '200vh' }}>
    <p>Page</p>
    <Modal layout="drawer" title="Filters" closeLabel="Close filters" onClose={() => {}}>
      <div data-section style={{ height: 68 }} />
    </Modal>
  </main>
)
