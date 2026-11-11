import {
  Fragment,
  cloneElement,
  useContext,
  useEffect,
  useRef,
  type ComponentType,
  type AnchorHTMLAttributes,
  type ButtonHTMLAttributes,
  type HTMLAttributes,
  type ReactNode
} from 'react'
import { FigmaInteractions } from './figma-interactions'
import { iconComponents, isIconComponentName, type IconProps } from '../../foundations/icons'
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
  props: SemanticProps,
  label = semantic.label
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
      'aria-label': label
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

// The consumer's label for a count shown inside this node, which then names the node's action.
function countLabel(node: ViewNode, countLabels: Record<string, string>): string | undefined {
  if (node.slot && countLabels[node.slot]) return countLabels[node.slot]
  for (const child of node.children ?? []) {
    const label = countLabel(child, countLabels)
    if (label) return label
  }
}

function renderNode(
  node: ViewNode,
  slots: Record<string, ReactNode>,
  interaction: React.ContextType<typeof FigmaInteractions>,
  semanticProps: SemanticProps = {},
  countLabels: Record<string, string> = {}
): ReactNode {
  // The key is passed on its own: React rejects a key spread into JSX inside a props object.
  const key = node.path || 'root'
  const common = {
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
      <div key={key} {...common}>
        {leading?.map((child) => renderNode(child, slots, interaction, semanticProps, countLabels))}
        <span key="native-field" className={styles.fieldControl}>
          {cloneElement(interaction.field.control, {
            className: `${text?.className ?? ''} ${interaction.field.control.props.className ?? ''}`
          })}
        </span>
        {trailing?.map((child) =>
          renderNode(child, slots, interaction, semanticProps, countLabels)
        )}
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
        key={key}
        {...common}
        className={`${common.className} ${styles.action}`}
        type="button"
        aria-label={action.label}
        aria-pressed={action.pressed}
        onClick={action.onActivate}
      >
        {node.children?.map((child) => renderNode(child, slots, null, semanticProps, countLabels))}
      </button>
    )
  if (node.semantic && node.kind === 'box')
    return renderSemantic(
      node.semantic,
      { key, ...common },
      node.children?.map((child) =>
        renderNode(child, slots, interaction, semanticProps, countLabels)
      ),
      semanticProps,
      node.semantic.role === 'action'
        ? (countLabel(node, countLabels) ?? node.semantic.label)
        : undefined
    )
  const override = node.slot ? slots[node.slot] : undefined
  switch (node.kind) {
    case 'text':
      return (
        <span key={key} {...common}>
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
      const Icon: ComponentType<IconProps> | undefined =
        node.icon && isIconComponentName(node.icon.component)
          ? iconComponents[node.icon.component]
          : undefined
      return (
        <span key={key} {...common}>
          {override ??
            (Icon && node.icon ? <Icon size={node.icon.size} {...node.icon.props} /> : null)}
        </span>
      )
    }
    case 'ref': {
      const Component = node.ref ? registry.get(node.ref.component) : undefined
      return (
        <div key={key} {...common}>
          {Component ? <Component {...node.ref?.props} /> : null}
        </div>
      )
    }
    case 'glyph':
      return (
        <div
          key={key}
          {...common}
          title={`${node.glyph?.name ?? node.name}: ${node.glyph?.reason ?? ''}`}
        />
      )
    default:
      return (
        <div
          key={key}
          {...common}
          role={interaction?.tabList === node.name ? 'tablist' : undefined}
        >
          {node.children?.map((child) =>
            renderNode(child, slots, interaction, semanticProps, countLabels)
          )}
        </div>
      )
  }
}

type Attributes = Omit<HTMLAttributes<HTMLElement>, 'children'>

// The runtime's own parameter names. A generated component already keeps a caller from overriding them (its props
// arrive only as `attributes`); an untyped caller can still pass one, so it is dropped here instead of reaching the DOM
// as an attribute. Public props (forceState, disabled, className, style, handlers, aria-*, data-*, ref) are untouched.
// The list is the runtime's: the installed library carries this file without the generator.
const RESERVED_PROPS: ReadonlySet<string> = new Set([
  'as',
  'action',
  'attributes',
  'component',
  'counts',
  'defaultKey',
  'select',
  'semanticProps',
  'slots',
  'suppressed',
  'views'
])

// Everything except `attributes` comes from the generated component. The caller's own props arrive only through
// `attributes`, so no prop a caller passes can replace the generated configuration.
// A count must be a non-negative integer and comes with its label; the label replaces the action's accessible name,
// so it has to say the count itself (for example "12 notifications").
function resolveCounts(
  slots: Record<string, ReactNode>,
  counts: Record<string, string | undefined>,
  component = 'A generated component'
): Record<string, string> {
  const labels: Record<string, string> = {}
  for (const [role, label] of Object.entries(counts)) {
    const value = slots[role]
    if (value === undefined) continue
    if (typeof value !== 'number' || !Number.isInteger(value) || value < 0)
      throw new Error(
        `${component}: ${role} must be a non-negative integer, got ${JSON.stringify(value)}`
      )
    if (typeof label !== 'string' || !label.trim())
      throw new Error(`${component}: ${role} needs a non-empty accessible label`)
    labels[role] = label
  }
  return labels
}

type ActionProps = {
  children?: ReactNode
  leadingIcon?: ReactNode
  trailingIcon?: ReactNode
  iconOnly?: boolean
  appearance?: 'inverse' | 'iconOnly'
} & (
  | (ButtonHTMLAttributes<HTMLButtonElement> & { href?: never })
  | (Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'type'> & {
      href: string
      disabled?: boolean
      type?: never
    })
)

function safeRel(rel: string | undefined): string {
  return [...new Set([...(rel ?? '').split(/\s+/).filter(Boolean), 'noopener', 'noreferrer'])].join(
    ' '
  )
}

type FigmaViewProps = {
  as?: 'button' | 'div'
  action?: ActionProps
  views: Record<string, ViewNode>
  select: Selection
  slots: Record<string, ReactNode>
  forceState?: ForcedState
  disabled?: boolean
  // Values for the href/onActivate props a component's opt-in semantics declare.
  semanticProps?: SemanticProps
  // The component's name, for the error an uncaptured variant selection raises.
  component?: string
  // Count text roles and the label each takes (the consumer's accessible text, which carries the count).
  counts?: Record<string, string | undefined>
  // The caller's remaining props, forwarded to the root element.
  attributes?: Attributes
  // Props the component's type forbids (text overrides it does not expose). An untyped caller may still pass them
  // inside `attributes`, so they are dropped before the DOM.
  suppressed?: readonly string[]
}

function FigmaView({
  as: Tag = 'div',
  action,
  views,
  select,
  slots,
  forceState,
  disabled,
  semanticProps = {},
  component,
  counts = {},
  attributes = {},
  suppressed = []
}: FigmaViewProps) {
  const interaction = useContext(FigmaInteractions)
  const trigger = useRef<HTMLButtonElement>(null)
  const surface = useRef<HTMLDivElement>(null)
  const { className, ...rest } = Object.fromEntries(
    Object.entries(attributes).filter(
      ([name]) => !suppressed.includes(name) && !RESERVED_PROPS.has(name)
    )
  ) as Attributes
  const view = pickView(views, select, component)
  const countLabels = resolveCounts(slots, counts, component)
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
            .map((child) => renderNode(child, slots, null, semanticProps, countLabels))}
        </button>
        {view.children
          ?.slice(disclosure.headerCount)
          .map((child) => renderNode(child, slots, interaction, semanticProps, countLabels))}
      </div>
    )
  if (action && Tag === 'button') {
    const {
      children,
      leadingIcon,
      trailingIcon,
      iconOnly,
      appearance,
      className: actionClass,
      ...native
    } = Object.fromEntries(
      Object.entries(action).filter(
        ([name]) => !RESERVED_PROPS.has(name) && !suppressed.includes(name)
      )
    ) as ActionProps
    const compact = iconOnly || appearance === 'iconOnly'
    if (compact && !native['aria-label']?.trim() && !native['aria-labelledby']?.trim())
      throw new Error(`${component ?? 'Button'}: iconOnly requires an accessible name`)
    const content = (
      <>
        {leadingIcon != null && (
          <span className={styles.controlIcon} aria-hidden="true">
            {leadingIcon}
          </span>
        )}
        {!compact &&
          view.children?.map((child) =>
            renderNode(
              child,
              children === undefined ? slots : { ...slots, label: children },
              interaction,
              semanticProps,
              countLabels
            )
          )}
        {trailingIcon != null && (
          <span className={styles.controlIcon} aria-hidden="true">
            {trailingIcon}
          </span>
        )}
        {compact && leadingIcon == null && trailingIcon == null && children}
      </>
    )
    const common = {
      ...rest,
      className: [
        styles.node,
        styles.control,
        view.className,
        compact && styles.iconOnly,
        appearance === 'inverse' && styles.inverse,
        className,
        actionClass
      ]
        .filter(Boolean)
        .join(' '),
      'data-n': '',
      'data-force-state': forceState
    }
    const unavailable = native['aria-disabled'] === true || native['aria-disabled'] === 'true'
    if (typeof native.href === 'string') {
      const { href, target, rel, onClick, onAuxClick, disabled: actionDisabled, ...anchor } = native
      const inactive = disabled || actionDisabled
      return (
        <a
          {...common}
          {...anchor}
          href={inactive ? undefined : href}
          role={inactive ? (native.role ?? 'link') : native.role}
          target={target}
          rel={target === '_blank' ? safeRel(rel) : rel}
          aria-disabled={inactive || native['aria-disabled']}
          tabIndex={inactive ? -1 : native.tabIndex}
          onClick={(event) => {
            if (inactive || unavailable) {
              event.preventDefault()
              return
            }
            onClick?.(event)
          }}
          onAuxClick={(event) => {
            if (inactive || unavailable) {
              event.preventDefault()
              return
            }
            onAuxClick?.(event)
          }}
        >
          {content}
        </a>
      )
    }
    const {
      type = 'button',
      onClick,
      onAuxClick,
      ...button
    } = native as ButtonHTMLAttributes<HTMLButtonElement>
    return (
      <button
        {...common}
        {...button}
        type={type}
        disabled={disabled || button.disabled}
        onClick={(event) => {
          if (unavailable) {
            event.preventDefault()
            return
          }
          onClick?.(event)
        }}
        onAuxClick={(event) => {
          if (unavailable) {
            event.preventDefault()
            return
          }
          onAuxClick?.(event)
        }}
      >
        {content}
      </button>
    )
  }
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
      {view.children?.map((child) =>
        renderNode(child, slots, interaction, semanticProps, countLabels)
      )}
    </Root>
  )
}

export { FigmaView, registerComponent }
export type { ActionProps, ForcedState, ViewNode }
