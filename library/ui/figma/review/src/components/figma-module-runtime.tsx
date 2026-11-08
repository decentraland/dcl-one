import {
  Fragment,
  cloneElement,
  useContext,
  useEffect,
  useRef,
  type HTMLAttributes,
  type ReactNode
} from 'react'
import { FigmaInteractions } from './figma-interactions'
import { iconComponents } from '../generated/icons'
import {
  pickView,
  registerComponent,
  registry,
  type ForcedState,
  type Selection
} from './figma-view-state'
import styles from './figma-module-runtime.module.css'

type ViewNode = {
  kind: 'box' | 'text' | 'icon' | 'ref' | 'glyph'
  path: string
  name: string
  className: string
  children?: ViewNode[]
  text?: string
  textRuns?: { text: string; className: string }[]
  slot?: string
  icon?: { component: string; props?: Record<string, string | boolean>; size: number }
  ref?: { component: string; props?: Record<string, string | boolean> }
  glyph?: { name: string; reason: string }
  semantic?: Semantic
}

// Opt-in accessible semantics the generator emits from figma.config.json. Nodes without it render
// exactly as before.
type Semantic = {
  role: 'navigation' | 'list' | 'action'
  label?: string
  href?: string
  onActivate?: string
  listItem?: boolean
  fallback?: 'button' | 'none'
}
type SemanticProps = Record<string, string | (() => void) | undefined>

function findTextNode(node: ViewNode): ViewNode | undefined {
  if (node.kind === 'text') return node
  for (const child of node.children ?? []) {
    const text = findTextNode(child)
    if (text) return text
  }
}

function textContent(node: ViewNode, slots: Record<string, ReactNode>): string {
  const override = node.slot ? slots[node.slot] : undefined
  return node.kind === 'text'
    ? String(override ?? node.text ?? '')
    : (node.children ?? [])
        .map((child) => textContent(child, slots))
        .join(' ')
        .trim()
}

// The node keeps its own class (layout and look); the semantic element only replaces the div.
// A list item wraps an action with display: contents, so the box it draws does not change.
function renderSemantic(
  semantic: Semantic,
  { key, ...attributes }: { key: string; className: string; 'data-n': string },
  children: ReactNode,
  props: SemanticProps
): ReactNode {
  let element: ReactNode
  if (semantic.role === 'navigation')
    element = (
      <nav {...attributes} aria-label={semantic.label}>
        {children}
      </nav>
    )
  else if (semantic.role === 'list')
    element = (
      <ul {...attributes} className={`${attributes.className} ${styles.semanticList}`}>
        {children}
      </ul>
    )
  else {
    const href = semantic.href ? props[semantic.href] : undefined
    const handler = semantic.onActivate ? props[semantic.onActivate] : undefined
    const onClick = typeof handler === 'function' ? handler : undefined
    const action = {
      ...attributes,
      className: `${attributes.className} ${styles.action} ${styles.semanticAction}`,
      'aria-label': semantic.label
    }
    if (typeof href === 'string')
      element = (
        <a {...action} href={href} onClick={onClick}>
          {children}
        </a>
      )
    else if (onClick || semantic.fallback !== 'none')
      element = (
        <button {...action} type="button" onClick={onClick}>
          {children}
        </button>
      )
    else element = <div {...attributes}>{children}</div>
  }
  return semantic.listItem ? (
    <li key={key} className={styles.semanticItem}>
      {element}
    </li>
  ) : (
    <Fragment key={key}>{element}</Fragment>
  )
}

function renderNode(
  node: ViewNode,
  slots: Record<string, ReactNode>,
  interaction: React.ContextType<typeof FigmaInteractions>,
  semanticProps: SemanticProps = {}
): ReactNode {
  const common = {
    key: node.path || 'root',
    className: `${styles.node} ${node.className}`,
    'data-n': node.path
  }
  if (interaction?.field?.container === node.name) {
    const text = findTextNode(node)
    const leading = node.children?.filter(
      (child) => child.name === 'Icon start' || child.name === 'general/search'
    )
    const trailing = node.children?.filter(
      (child) =>
        child.name === 'Icon end' ||
        child.name === 'general/preview' ||
        child.name === 'general/hide-ui'
    )
    return (
      <div {...common}>
        {leading?.map((child) => renderNode(child, slots, interaction, semanticProps))}
        <span key="native-field" className={styles.fieldControl}>
          {cloneElement(interaction.field.control, {
            className: `${text?.className ?? ''} ${interaction.field.control.props.className ?? ''}`
          })}
        </span>
        {trailing?.map((child) => renderNode(child, slots, interaction, semanticProps))}
      </div>
    )
  }
  const replacement = interaction?.replacement?.(node.name, node.path, textContent(node, slots))
  if (replacement !== undefined)
    return (
      <FigmaInteractions.Provider key={node.path} value={null}>
        {replacement}
      </FigmaInteractions.Provider>
    )
  const action = interaction?.action?.(node.name, node.path, textContent(node, slots))
  if (action)
    return (
      <button
        {...common}
        className={`${common.className} ${styles.action}`}
        type="button"
        aria-label={action.label}
        aria-pressed={action.pressed}
        onClick={action.onActivate}
      >
        {node.children?.map((child) => renderNode(child, slots, null, semanticProps))}
      </button>
    )
  if (node.semantic && node.kind === 'box')
    return renderSemantic(
      node.semantic,
      common,
      node.children?.map((child) => renderNode(child, slots, interaction, semanticProps)),
      semanticProps
    )
  const override = node.slot ? slots[node.slot] : undefined
  switch (node.kind) {
    case 'text':
      return (
        <span {...common}>
          {override ??
            (node.textRuns
              ? node.textRuns.map((run, index) => (
                  <span key={index} className={run.className}>
                    {run.text}
                  </span>
                ))
              : node.text)}
        </span>
      )
    case 'icon': {
      const Icon = node.icon ? iconComponents[node.icon.component] : undefined
      return (
        <span {...common}>
          {override ??
            (Icon && node.icon ? <Icon size={node.icon.size} {...node.icon.props} /> : null)}
        </span>
      )
    }
    case 'ref': {
      const Component = node.ref ? registry.get(node.ref.component) : undefined
      return <div {...common}>{Component ? <Component {...node.ref?.props} /> : null}</div>
    }
    case 'glyph':
      return (
        <div {...common} title={`${node.glyph?.name ?? node.name}: ${node.glyph?.reason ?? ''}`} />
      )
    default:
      return (
        <div {...common} role={interaction?.tabList === node.name ? 'tablist' : undefined}>
          {node.children?.map((child) => renderNode(child, slots, interaction, semanticProps))}
        </div>
      )
  }
}

type Attributes = Omit<HTMLAttributes<HTMLElement>, 'children'>

// Everything except `attributes` comes from the generated component. The caller's own props arrive only through
// `attributes`, so no prop a caller passes can replace the generated configuration.
type FigmaViewProps = {
  as?: 'button' | 'div'
  views: Record<string, ViewNode>
  select: Selection
  slots: Record<string, ReactNode>
  forceState?: ForcedState
  disabled?: boolean
  // Values for the href/onActivate props a component's opt-in semantics declare.
  semanticProps?: SemanticProps
  // The component's name, for the error an uncaptured variant selection raises.
  component?: string
  // The caller's remaining props, forwarded to the root element.
  attributes?: Attributes
  // Props the component's type forbids (text overrides it does not expose). An untyped caller may still pass them
  // inside `attributes`, so they are dropped before the DOM.
  suppressed?: readonly string[]
}

function FigmaView({
  as: Tag = 'div',
  views,
  select,
  slots,
  forceState,
  disabled,
  semanticProps = {},
  component,
  attributes = {},
  suppressed = []
}: FigmaViewProps) {
  const interaction = useContext(FigmaInteractions)
  const trigger = useRef<HTMLButtonElement>(null)
  const surface = useRef<HTMLDivElement>(null)
  const { className, ...rest } = Object.fromEntries(
    Object.entries(attributes).filter(([name]) => !suppressed.includes(name))
  ) as Attributes
  const view = pickView(views, select, component)
  const disclosure = interaction?.disclosure
  useEffect(() => {
    if (!disclosure?.open) return
    const closeOutside = (event: PointerEvent) => {
      if (event.target instanceof Node && !surface.current?.contains(event.target))
        disclosure.onClose()
    }
    document.addEventListener('pointerdown', closeOutside)
    return () => document.removeEventListener('pointerdown', closeOutside)
  }, [disclosure])
  if (disclosure)
    return (
      <div
        {...rest}
        ref={surface}
        onBlur={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget)) disclosure.onClose()
        }}
        onClick={(event) => {
          if (event.target instanceof Element && event.target.closest('button') !== trigger.current)
            trigger.current?.focus()
        }}
        className={[styles.node, styles.disclosure, view.className, className]
          .filter(Boolean)
          .join(' ')}
        data-n=""
        data-force-state={forceState}
        data-interaction-mode="controlled"
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault()
            disclosure.onClose()
            trigger.current?.focus()
          }
        }}
      >
        <button
          ref={trigger}
          type="button"
          className={styles.trigger}
          aria-label={disclosure.label}
          aria-expanded={disclosure.open}
          disabled={disabled}
          onClick={disclosure.onToggle}
        >
          {view.children
            ?.slice(0, disclosure.headerCount)
            .map((child) => renderNode(child, slots, null, semanticProps))}
        </button>
        {view.children
          ?.slice(disclosure.headerCount)
          .map((child) => renderNode(child, slots, interaction, semanticProps))}
      </div>
    )
  // A root navigation landmark replaces the default div; button roots keep their element.
  const Root = view.semantic?.role === 'navigation' && Tag === 'div' ? 'nav' : Tag
  return (
    <Root
      aria-label={Root === 'nav' ? view.semantic?.label : undefined}
      {...rest}
      className={[styles.node, view.className, className].filter(Boolean).join(' ')}
      data-n=""
      data-force-state={forceState}
      {...(Root === 'button'
        ? { type: 'button', disabled }
        : { 'aria-disabled': disabled || undefined })}
    >
      {view.children?.map((child) => renderNode(child, slots, interaction, semanticProps))}
    </Root>
  )
}

export { FigmaView, registerComponent }
export type { ForcedState, ViewNode }
