import { createRoot } from 'react-dom/client'
import { useEffect, useState } from 'react'
import { InputExplorerNames } from '@dcl/one-ui'
function App() {
  const q = new URLSearchParams(location.search),
    cancel = q.get('cancel'),
    external = q.has('external')
  const [mounted, setMounted] = useState(true)
  useEffect(() => {
    const late = (e: Event) => {
      if (cancel === 'document') e.preventDefault()
      document.body.dataset.canceled = String(e.defaultPrevented)
    }
    document.addEventListener('reset', late)
    return () => document.removeEventListener('reset', late)
  }, [cancel])
  const Field = InputExplorerNames
  const input = mounted ? (
    <Field
      label="Password"
      type="password"
      allowPasswordReveal
      defaultValue="secret"
      form={external ? 'owner' : undefined}
    />
  ) : null
  return (
    <>
      <form
        id="owner"
        onReset={(e) => {
          if (cancel === 'form') e.preventDefault()
          if (q.has('unmount')) setMounted(false)
        }}
      >
        {external ? null : input}
        <button id="reset" type="reset">
          Reset
        </button>
      </form>
      {external ? input : null}
    </>
  )
}
createRoot(document.getElementById('root')!).render(<App />)
