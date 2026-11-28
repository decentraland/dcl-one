import { createRoot } from 'react-dom/client'
import { ContextMenu, type ContextMenuItem } from '@dcl/one-ui'
import '../../foundations/foundations.css'

// A row of a ContextMenu that opens a list beside it, as the Explorer HUD passport's options menu draws Invite to
// Community. `#right` has room on its right. `#left` ends at a clipping edge just past the menu, so its list opens
// on the row's left.
const items: ContextMenuItem[] = [
  {
    kind: 'submenu',
    label: 'Invite to Community',
    items: [{ label: 'Pixel Builders' }, { label: 'Neon Club' }]
  },
  { label: 'Block', danger: true }
]

createRoot(document.getElementById('root')!).render(
  <div style={{ display: 'flex' }}>
    <div id="right" style={{ padding: 40, width: 640 }}>
      <ContextMenu aria-label="Right" items={items} />
    </div>
    <div
      id="left"
      style={{
        display: 'flex',
        justifyContent: 'flex-end',
        alignItems: 'flex-start',
        width: 640,
        padding: '40px 8px 240px',
        overflow: 'hidden'
      }}
    >
      <ContextMenu aria-label="Left" items={items} />
    </div>
  </div>
)
