import { useCallback, useMemo, useState } from 'react'
import { createPanelStorage, loadEditorLayout, saveEditorLayout } from '~/lib/itemEditor'

/** The sidebar mode and the panel-size storage, both remembered across visits. */
export function useEditorLayout() {
  const [sidebarCollapsed, setCollapsed] = useState(() => loadEditorLayout().sidebarCollapsed ?? false)
  const toggleSidebar = useCallback(() => {
    setCollapsed(collapsed => {
      saveEditorLayout({ sidebarCollapsed: !collapsed })
      return !collapsed
    })
  }, [])
  const panelStorage = useMemo(() => createPanelStorage(), [])
  return { sidebarCollapsed, toggleSidebar, panelStorage }
}
