import { createRoot } from 'react-dom/client'
import { ShopSearch } from '@dcl/one-ui'
import '../../foundations/foundations.css'

const noop = () => {}
createRoot(document.getElementById('root')!).render(
  <ShopSearch
    inputRef={null}
    value="hat"
    open
    active={-1}
    recent={[]}
    rows={[]}
    loading={false}
    failed={false}
    empty
    onOpenChange={noop}
    onChange={noop}
    onSubmit={noop}
    onKeyDown={noop}
    onClear={noop}
    onClearRecent={noop}
    onRemoveRecent={noop}
    onRetry={noop}
  />
)
