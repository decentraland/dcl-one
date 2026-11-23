import { createRoot } from 'react-dom/client'
import { useState } from 'react'
import { Button } from '@dcl/one-ui'
import '../../foundations/foundations.css'
const leading = <span style={{ display: 'block', width: 20, height: 20 }} data-marker="leading" />,
  trailing = <span style={{ display: 'block', width: 20, height: 20 }} data-marker="trailing" />
function Fixture() {
  const [loading, setLoading] = useState(false),
    [calls, setCalls] = useState(0)
  return (
    <main
      style={{
        padding: 16,
        display: 'flex',
        flexDirection: 'column',
        gap: 16,
        alignItems: 'flex-start',
        background: 'var(--dcl-neutrals-shadow)'
      }}
    >
      {(['intrinsic', 'fill'] as const).map((contentLayout) => (
        <Button
          key={contentLayout}
          data-case={contentLayout}
          contentLayout={contentLayout}
          size="md"
          style={{ width: 230 }}
          leadingIcon={leading}
          trailingIcon={trailing}
        >
          Action
        </Button>
      ))}
      <Button
        data-case="long"
        contentLayout="fill"
        size="md"
        style={{ width: 230 }}
        leadingIcon={leading}
        trailingIcon={trailing}
      >
        A long action label that should remain named without covering either icon
      </Button>
      <Button data-case="compact" contentLayout="fill" size="md" iconOnly aria-label="Open">
        {leading}
      </Button>
      <Button
        data-case="loading"
        contentLayout="fill"
        size="md"
        style={{ width: 230 }}
        leadingIcon={leading}
        trailingIcon={trailing}
        loading={loading}
        onClick={() => {
          setCalls(calls + 1)
          setLoading(true)
        }}
      >
        Save record
      </Button>
      <Button
        data-case="link"
        contentLayout="fill"
        href="#target"
        size="md"
        style={{ width: 230 }}
        leadingIcon={leading}
        trailingIcon={trailing}
      >
        Native destination
      </Button>
      {(['xl', 'lg', 'md', 'sm'] as const).map((size) => (
        <Button key={size} variant="secondaryLight" size={size} startIcon={<svg />} loading>
          Remind me
        </Button>
      ))}
      <output>{calls}</output>
      <span id="target">Destination</span>
    </main>
  )
}
createRoot(document.getElementById('root')!).render(<Fixture />)
