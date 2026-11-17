import { createRoot } from 'react-dom/client'
import { useEffect, useState } from 'react'
import { InputExplorerNames } from '@dcl/one-ui'
function App() {
  const q = new URLSearchParams(location.search),
    cancel = q.get('cancel'),
    external = q.has('external')
  const [mounted, setMounted] = useState(true)
  const [revealAllowed, setRevealAllowed] = useState(true)
  const [fieldType, setFieldType] = useState<'password' | 'text'>('password')
  const [disabled, setDisabled] = useState(false)
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
      type={fieldType}
      allowPasswordReveal={revealAllowed}
      disabled={disabled}
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
      <button id="allow" type="button" onClick={() => setRevealAllowed((value) => !value)}>
        Toggle reveal permission
      </button>
      <button
        id="field-type"
        type="button"
        onClick={() => setFieldType((value) => (value === 'password' ? 'text' : 'password'))}
      >
        Toggle native type
      </button>
      <button id="disabled" type="button" onClick={() => setDisabled((value) => !value)}>
        Toggle disabled
      </button>
    </>
  )
}
createRoot(document.getElementById('root')!).render(<App />)
