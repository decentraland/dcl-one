import { useMemo, useState } from 'react'
import { createPortal } from 'react-dom'
import {
  ArrowDropDownIcon as ChevronIcon,
  ChevronRightIcon as CaretIcon,
  CloseIcon as ClearIcon
} from '../../../Icons'
import { useListbox } from '../../../Select/useListbox'
import { type BoneNode, type BoneTreeNode } from '@/data/lib/creators/springBones'
import styles from './BoneTreePicker.module.css'

/** The picker needs a value for "no bone"; the schema's own value for it is `undefined`. */
export const NO_BONE = '__none__'

type Props = {
  tree: BoneTreeNode[]
  value: string | undefined
  onChange: (name: string | undefined) => void
  /** Label of the "no bone" row, also shown in the trigger while nothing is picked. */
  noneLabel: string
  clearLabel: string
  /** Bones of this type are listed for context but cannot be picked. */
  disabledType?: BoneNode['type']
  disabled?: boolean
  ariaLabel?: string
  testId?: string
}

const LIST_MIN_WIDTH = 230
const ROW_INDENT = 12
const ROW_PADDING = 10

type Row = { bone: BoneNode; depth: number; hasChildren: boolean; expanded: boolean; selectable: boolean }

/** Bone hierarchy dropdown: the model's skeleton as a collapsible tree, one bone selectable. */
export function BoneTreePicker({
  tree,
  value,
  onChange,
  noneLabel,
  clearLabel,
  disabledType,
  disabled = false,
  ariaLabel,
  testId = 'bone-tree'
}: Props) {
  const [collapsed, setCollapsed] = useState<Set<number>>(new Set())

  const rows = useMemo(() => {
    const flattened: Row[] = []
    const walk = (nodes: BoneTreeNode[], depth: number) => {
      for (const node of nodes) {
        const expanded = !collapsed.has(node.bone.nodeId)
        flattened.push({
          bone: node.bone,
          depth,
          hasChildren: node.children.length > 0,
          expanded,
          selectable: node.bone.type !== disabledType
        })
        if (expanded) walk(node.children, depth + 1)
      }
    }
    walk(tree, 0)
    return flattened
  }, [tree, collapsed, disabledType])

  const options = useMemo(
    () => [{ value: NO_BONE }, ...rows.filter(row => row.selectable).map(row => ({ value: row.bone.name }))],
    [rows]
  )

  const { open, active, setActive, wrapRef, listRef, listStyle, listId, toggle, pick, onKeyDown } = useListbox({
    options,
    current: value ?? NO_BONE,
    onPick: name => onChange(name === NO_BONE ? undefined : name),
    minWidth: LIST_MIN_WIDTH
  })

  function toggleNode(nodeId: number) {
    setCollapsed(previous => {
      const next = new Set(previous)
      if (!next.delete(nodeId)) next.add(nodeId)
      return next
    })
  }

  function onRowClick(row: Row) {
    if (row.selectable) pick(row.bone.name)
    else if (row.hasChildren) toggleNode(row.bone.nodeId)
  }

  return (
    <div className={styles.wrap} ref={wrapRef} onKeyDown={onKeyDown}>
      <button
        className={styles.trigger}
        type="button"
        role="combobox"
        aria-haspopup="tree"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-label={ariaLabel}
        disabled={disabled}
        data-testid={testId}
        data-placeholder={value ? undefined : true}
        onClick={toggle}
      >
        <span title={value}>{value ?? noneLabel}</span>
        {!value && <ChevronIcon />}
      </button>
      {value && !disabled && (
        <button
          className={styles.clear}
          type="button"
          aria-label={clearLabel}
          data-testid={`${testId}-clear`}
          onClick={() => onChange(undefined)}
        >
          <ClearIcon />
        </button>
      )}
      {open &&
        createPortal(
          <ul
            className={styles.tree}
            ref={listRef}
            role="tree"
            id={listId}
            data-testid={`${testId}-tree`}
            style={listStyle}
            onKeyDown={onKeyDown}
          >
            <li
              className={styles.row}
              role="treeitem"
              aria-selected={!value}
              data-active={active === NO_BONE || undefined}
              data-testid={`${testId}-option-${NO_BONE}`}
              style={{ paddingLeft: ROW_PADDING }}
              onMouseEnter={() => setActive(NO_BONE)}
              onClick={() => pick(NO_BONE)}
            >
              <span className={styles.caret} data-hidden />
              <span>{noneLabel}</span>
            </li>
            {rows.map(row => (
              <li
                className={styles.row}
                key={row.bone.nodeId}
                role="treeitem"
                aria-selected={row.bone.name === value}
                aria-expanded={row.hasChildren ? row.expanded : undefined}
                aria-disabled={!row.selectable || undefined}
                data-active={(row.selectable && active === row.bone.name) || undefined}
                data-disabled={!row.selectable || undefined}
                data-testid={`${testId}-option-${row.bone.name}`}
                style={{ paddingLeft: ROW_PADDING + row.depth * ROW_INDENT }}
                onMouseEnter={() => row.selectable && setActive(row.bone.name)}
                onClick={() => onRowClick(row)}
              >
                <span
                  className={styles.caret}
                  data-hidden={!row.hasChildren || undefined}
                  data-open={row.expanded || undefined}
                  data-testid={`${testId}-toggle-${row.bone.name}`}
                  onClick={event => {
                    event.stopPropagation()
                    toggleNode(row.bone.nodeId)
                  }}
                >
                  <CaretIcon />
                </span>
                <span title={row.bone.name}>{row.bone.name}</span>
              </li>
            ))}
          </ul>,
          document.body
        )}
    </div>
  )
}
