import { createRoot } from 'react-dom/client'
import { Modal } from '../../components/Modal/Modal'
import '../../foundations/foundations.css'

// `?layout=message|detail|scene|page&tall=1`: one dialog over a page taller than the screen.
const query = new URLSearchParams(location.search)
const kind = query.get('layout') ?? 'message'
const tall = query.has('tall')
const layout = kind === 'page' ? 'scene' : (kind as 'message' | 'detail' | 'scene')
const cover = <div data-cover style={{ aspectRatio: '880 / 449', background: '#333' }} />

document.body.style.margin = '0'

createRoot(document.getElementById('root')!).render(
  <main style={{ minHeight: '200vh' }}>
    <p>Page</p>
    <Modal
      layout={layout}
      pageContent={kind === 'page'}
      title="Genesis Plaza"
      description={layout === 'message' ? 'You will return to Genesis Plaza.' : undefined}
      image={layout === 'message' ? undefined : cover}
      onClose={() => {}}
    >
      <div data-body style={{ height: tall ? 1200 : 120 }} />
    </Modal>
  </main>
)
