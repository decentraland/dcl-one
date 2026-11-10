import { useEffect, useRef, useState, type ComponentType, type CSSProperties } from 'react'
import { FigmaInteractions, type FigmaInteractionBindings } from '../components/figma-interactions'
import { Tabs } from '../generated/components/tabs-bar/Tabs'
import styles from './InteractivePreview.module.css'
import { emailState, explorerState, restoreCaret, searchState } from './InteractivePreview.helpers'
import controls from './ReviewGallery.module.css'

function InteractivePreview({
  name,
  component: Component,
  properties
}: {
  name: string
  component: ComponentType<Record<string, unknown>>
  properties: Record<string, unknown>
}) {
  const canvas = useRef<HTMLDivElement>(null)
  const [overflow, setOverflow] = useState(0)
  const [value, setValue] = useState('')
  const [focused, setFocused] = useState(false)
  const [touched, setTouched] = useState(false)
  const [submitted, setSubmitted] = useState(false)
  const [open, setOpen] = useState(false)
  const [reveal, setReveal] = useState(false)
  const [dismissed, setDismissed] = useState(false)
  const [message, setMessage] = useState('')
  const [activeTab, setActiveTab] = useState(0)
  const [selected, setSelected] = useState(properties.selected === true)
  const email = name === 'InputWebCreatorTools'
  const explorer = name === 'InputExplorerNames'
  const search = name === 'SearchBar'
  const modal = name === 'ShopModals'
  const tabsBar = name === 'TabsBar'
  const field = email || explorer || search || modal
  const password = explorer && String(properties.variant).startsWith('password')
  const menuTab =
    name === 'NavMenuTab' && ['whatsOn', 'shop', 'learn'].includes(String(properties.tab))
  const menuDropdown = name === 'NavMenuDropdown' && properties.tab !== 'whatsOn'
  const disclosure =
    name === 'FooterAccordion' || name === 'NavMenuTabMobile' || menuTab || menuDropdown
  const disabled = properties.disabled === true
  const props: Record<string, unknown> = { ...properties, forceState: undefined }
  const bindings: FigmaInteractionBindings = {}
  const label = email
    ? 'Email'
    : password
      ? 'Password'
      : search
        ? 'Search'
        : modal
          ? 'Collection name'
          : 'Name'
  if (email) {
    props.state = emailState(value, focused, touched)
    props.label4 = `${value.length}/50`
  } else if (explorer) {
    props.variant = explorerState(value, focused, password, reveal)
    props[props.variant === 'focusedEditing' || props.variant === 'error' ? 'label3' : 'label2'] =
      `${value.length}/15`
  } else if (search) props.state = searchState(value, focused, submitted)
  else if (modal) props.label7 = `${value.length}/32`
  if (field)
    bindings.field = {
      container: email || modal ? 'Field' : search ? 'Frame 1000001701' : 'Input',
      control: (
        <input
          // Figma nests the field at a different depth in each state, so a state change remounts it.
          ref={(element) => {
            if (element && focused && document.activeElement !== element) {
              element.focus()
              restoreCaret(element, value.length)
            }
          }}
          className={styles.input}
          name="playground-field"
          aria-label={label}
          placeholder={
            email
              ? 'Enter your email'
              : password
                ? 'Enter Password'
                : search
                  ? 'Search'
                  : modal
                    ? 'e.g. Halloween'
                    : 'Write here'
          }
          type={email ? 'email' : password && !reveal ? 'password' : search ? 'search' : 'text'}
          maxLength={email ? 50 : modal ? 32 : undefined}
          disabled={disabled}
          value={value}
          aria-invalid={props.state === 'error' || props.variant === 'error'}
          onChange={(event) => {
            setValue(event.target.value)
            setSubmitted(false)
            setMessage('')
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              setTouched(true)
              if (search) setSubmitted(true)
              if (search)
                setMessage(
                  value.trim()
                    ? `Search submitted: ${value.trim()}. No search service is connected.`
                    : 'Enter a search term.'
                )
              else event.currentTarget.blur()
            }
          }}
          onFocus={() => setFocused(true)}
          onBlur={() => {
            setFocused(false)
            setTouched(true)
          }}
        />
      )
    }
  if (disclosure) {
    if (menuTab) {
      props.forceState = open ? 'hover' : undefined
      props.onPointerEnter = (event: React.PointerEvent) => {
        if (event.pointerType === 'mouse') setOpen(true)
      }
      props.onPointerLeave = (event: React.PointerEvent) => {
        if (event.pointerType === 'mouse') setOpen(false)
      }
    } else props.state = open ? 'open' : name === 'FooterAccordion' ? 'closed' : 'default'
    bindings.disclosure = {
      label:
        name === 'NavMenuTabMobile'
          ? 'Navigation menu'
          : String(
              properties.label ??
                (name === 'FooterAccordion' ? 'Footer links' : 'Navigation options')
            ),
      open,
      headerCount: menuTab ? 3 : open || name === 'NavMenuTabMobile' ? 1 : 2,
      onToggle: () => setOpen(!open),
      onClose: () => setOpen(false)
    }
  }
  bindings.action = (nodeName, _path, text) => {
    if (password && (nodeName === 'general/preview' || nodeName === 'general/hide-ui'))
      return {
        label: reveal ? 'Hide password' : 'Show password',
        pressed: reveal,
        onActivate: () => setReveal(!reveal)
      }
    if (disclosure && ['NavMenuItem', 'FooterLink', 'NavMenuDropdown'].includes(nodeName))
      return {
        label: text,
        onActivate: () => {
          setMessage(`Selected ${text}. Navigation belongs to the consuming application.`)
          setOpen(false)
        }
      }
    if (modal && ['Close Button', 'Cancel'].includes(nodeName))
      return {
        label: nodeName === 'Cancel' ? 'Cancel' : 'Close modal',
        onActivate: () => setDismissed(true)
      }
    if (modal && nodeName === 'Buy')
      return {
        label: 'Buy',
        onActivate: () =>
          setMessage(
            value.trim()
              ? `Collection name: ${value.trim()}. Purchase requires the consuming application; no transaction was sent.`
              : 'Enter a collection name before continuing.'
          )
      }
  }
  if (tabsBar) bindings.tabList = 'Frame 1000001573'
  if (tabsBar)
    bindings.replacement = (nodeName, path, text) => {
      if (nodeName !== 'Tabs') return undefined
      const index = Number(path.split('.').at(-1))
      return (
        <Tabs
          device={
            String(properties.viewport).toLowerCase().includes('mobile') ? 'mobile' : 'desktop'
          }
          label={text}
          selected={activeTab === index}
          role="tab"
          aria-selected={activeTab === index}
          tabIndex={activeTab === index ? 0 : -1}
          onClick={() => setActiveTab(index)}
          onKeyDown={(event) => {
            const tabs = Array.from(
              event.currentTarget.parentElement?.querySelectorAll<HTMLElement>('[role="tab"]') ?? []
            )
            const current = tabs.indexOf(event.currentTarget)
            const next =
              event.key === 'ArrowRight'
                ? (current + 1) % tabs.length
                : event.key === 'ArrowLeft'
                  ? (current + tabs.length - 1) % tabs.length
                  : event.key === 'Home'
                    ? 0
                    : event.key === 'End'
                      ? tabs.length - 1
                      : -1
            if (next >= 0) {
              event.preventDefault()
              setActiveTab(next)
              tabs[next]?.focus()
            }
          }}
        />
      )
    }
  if (name === 'Tabs') {
    props.selected = selected
    props['aria-pressed'] = selected
    props.onClick = () => {
      if (!disabled) setSelected(!selected)
    }
  }
  // The open disclosure and the SearchBar's history or results list draw outside the component box.
  const expanded = open || (search && props.state !== 'default')
  useEffect(() => {
    const surface = canvas.current?.firstElementChild
    if (!(surface instanceof HTMLElement)) return
    const observer = new ResizeObserver(() => {
      setOverflow(expanded ? Math.max(0, surface.scrollHeight - surface.clientHeight) : 0)
    })
    observer.observe(surface)
    return () => observer.disconnect()
  }, [expanded, props.state, dismissed, name])
  const supported = field || disclosure || name === 'Tabs' || tabsBar
  return (
    <section className={styles.playground} aria-label="Interactive playground">
      <h2>Interactive playground</h2>
      {!supported ? (
        <p>
          Interactive behavior is not implemented for this component. Use the comparison above to
          inspect its captured states.
        </p>
      ) : (
        <>
          <p>
            {search
              ? 'Type a query and press Enter. Results require a connected search service.'
              : modal
                ? 'Edit the collection name, cancel or close the modal. No purchase is performed.'
                : field
                  ? 'Type into the component to try focus, editing and validation.'
                  : disclosure
                    ? 'Click the component or use Enter, Space and Escape to open and close it.'
                    : 'Click or use the keyboard to toggle selection.'}
          </p>
          <div
            ref={canvas}
            className={styles.canvas}
            style={{ '--playground-overflow': `${overflow}px` } as CSSProperties}
          >
            {dismissed ? (
              <button
                type="button"
                className={controls.controlButton}
                onClick={() => setDismissed(false)}
              >
                Reopen modal
              </button>
            ) : (
              <FigmaInteractions.Provider value={bindings}>
                <Component {...props} />
              </FigmaInteractions.Provider>
            )}
          </div>
          {message && <p role="status">{message}</p>}
          <button
            className={controls.controlButton}
            type="button"
            onClick={() => {
              setValue('')
              setFocused(false)
              setTouched(false)
              setSubmitted(false)
              setOpen(false)
              setReveal(false)
              setDismissed(false)
              setMessage('')
              setSelected(properties.selected === true)
              setActiveTab(0)
            }}
          >
            Reset playground
          </button>
        </>
      )}
    </section>
  )
}

export { InteractivePreview }
