import { useMemo, useState, type CSSProperties } from 'react'
import { BodyShape, type SpringBoneParams } from '@dcl/schemas'
import { ActionsMenu, ActionsMenuDivider, ActionsMenuItem } from '@/features/creators/components/ActionsMenu'
import { Select } from '@/features/creators/components/Select'
import { useTranslation } from '@/data/lib/creators/intl'
import {
  buildBoneTree,
  buildSubtreeSizes,
  getChainRoots,
  getDefaultSpringBoneParams,
  MAX_SPRING_BONES,
  SPRING_BONE_RANGES as RANGES,
  pickTunableSpringBoneParams,
  sortByHierarchy,
  sumConfiguredBones,
  type BoneNode,
  type SpringBoneParamsByName
} from '@/data/lib/creators/springBones'
import { BoneTreePicker } from './BoneTreePicker'
import styles from './SpringBonesEditor.module.css'

export type SpringBonesModel = {
  hash: string
  bodyShapes: string[]
  bones: BoneNode[]
}

type Props = {
  /** Models with spring bones; two entries give a body-shape tab pair. */
  models: SpringBonesModel[]
  activeHash: string
  onActiveHashChange: (hash: string) => void
  params: SpringBoneParamsByName
  onChange: (params: SpringBoneParamsByName) => void
  disabled?: boolean
  testId?: string
}

type NumericParam = 'stiffness' | 'gravityPower' | 'drag'

const SLIDERS: Array<{ field: NumericParam; min: number; max: number }> = [
  { field: 'stiffness', min: RANGES.stiffness[0], max: RANGES.stiffness[1] },
  { field: 'gravityPower', min: RANGES.gravityPower[0], max: RANGES.gravityPower[1] },
  { field: 'drag', min: RANGES.drag[0], max: RANGES.drag[1] }
]

const AXES = ['x', 'y', 'z'] as const

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function shapeLabel(bodyShapes: string[]): 'male' | 'female' | 'both' {
  const male = bodyShapes.includes(BodyShape.MALE)
  const female = bodyShapes.includes(BodyShape.FEMALE)
  return male && female ? 'both' : female ? 'female' : 'male'
}

/** Per-chain physics params for a wearable's spring bones (legacy SpringBonesSection). */
export function SpringBonesEditor({
  models,
  activeHash,
  onActiveHashChange,
  params,
  onChange,
  disabled = false,
  testId = 'spring-bones'
}: Props) {
  const { t } = useTranslation()
  const [copied, setCopied] = useState<Omit<SpringBoneParams, 'isRoot'> | null>(null)
  const model = useMemo(
    () => models.find(candidate => candidate.hash === activeHash) ?? models[0],
    [models, activeHash]
  )
  const bones = useMemo(() => model?.bones ?? [], [model])
  const subtreeSizes = useMemo(() => buildSubtreeSizes(bones), [bones])
  const configured = useMemo(() => sumConfiguredBones(subtreeSizes, params), [subtreeSizes, params])
  const names = useMemo(() => sortByHierarchy(bones, Object.keys(params)), [bones, params])
  const addable = useMemo(
    () =>
      getChainRoots(bones)
        .filter(root => !(root.name in params) && configured + (subtreeSizes.get(root.name) ?? 1) <= MAX_SPRING_BONES)
        .map(root => ({
          value: root.name,
          label: root.name,
          trailing: t('item_editor.spring_bones.bones', { count: subtreeSizes.get(root.name) ?? 1 })
        })),
    [bones, params, configured, subtreeSizes, t]
  )

  // A chain's center is an ordinary avatar bone: the pivot its physics swing around.
  const boneTree = useMemo(() => buildBoneTree(bones), [bones])

  if (!model) return null

  function update(name: string, patch: Partial<SpringBoneParams>) {
    onChange({ ...params, [name]: { ...params[name], ...patch } })
  }

  function remove(name: string) {
    const next = { ...params }
    delete next[name]
    onChange(next)
  }

  function paste(name: string) {
    if (copied) update(name, copied)
  }

  return (
    <div className={styles.wrap} data-testid={testId}>
      {models.length > 1 && (
        <div className={styles.tabs} role="tablist" aria-label={t('item_editor.spring_bones.body_shape')}>
          {models.map(candidate => (
            <button
              className={styles.tab}
              key={candidate.hash}
              type="button"
              role="tab"
              aria-selected={candidate.hash === model.hash}
              data-testid={`${testId}-tab-${shapeLabel(candidate.bodyShapes)}`}
              onClick={() => onActiveHashChange(candidate.hash)}
            >
              {t(`item_editor.customizer.shape.${shapeLabel(candidate.bodyShapes)}`)}
            </button>
          ))}
        </div>
      )}
      <span
        className={styles.counter}
        data-over={configured > MAX_SPRING_BONES || undefined}
        data-testid={`${testId}-counter`}
      >
        {t('item_editor.spring_bones.counter', { count: configured, max: MAX_SPRING_BONES })}
      </span>
      {names.map(name => {
        const bone = params[name]
        return (
          <div className={styles.card} key={name} data-testid={`${testId}-bone-${name}`}>
            <div className={styles.cardHeader}>
              <span title={name}>{name}</span>
              <span className={styles.counter}>
                {t('item_editor.spring_bones.bones', { count: subtreeSizes.get(name) ?? 1 })}
              </span>
              <ActionsMenu
                label={t('item_editor.spring_bones.actions')}
                variant="row"
                tone="dark"
                testId={`${testId}-actions-${name}`}
              >
                <ActionsMenuItem
                  testId={`${testId}-copy-${name}`}
                  onClick={() => setCopied(pickTunableSpringBoneParams(bone))}
                >
                  {t('item_editor.spring_bones.copy_params')}
                </ActionsMenuItem>
                <ActionsMenuItem
                  testId={`${testId}-paste-${name}`}
                  disabled={disabled || !copied}
                  onClick={() => paste(name)}
                >
                  {t('item_editor.spring_bones.paste_params')}
                </ActionsMenuItem>
                <ActionsMenuDivider />
                <ActionsMenuItem testId={`${testId}-remove-${name}`} disabled={disabled} onClick={() => remove(name)}>
                  {t('item_editor.spring_bones.remove')}
                </ActionsMenuItem>
              </ActionsMenu>
            </div>
            {SLIDERS.map(({ field, min, max }) => (
              <label className={styles.param} key={field}>
                <span>{t(`item_editor.spring_bones.${field}`)}</span>
                <input
                  type="range"
                  min={min}
                  max={max}
                  step={0.01}
                  value={bone[field]}
                  disabled={disabled}
                  style={{ '--fill': `${((bone[field] - min) / (max - min)) * 100}%` } as CSSProperties}
                  aria-label={t(`item_editor.spring_bones.${field}`)}
                  data-testid={`${testId}-${name}-${field}`}
                  onChange={event => update(name, { [field]: Number(event.target.value) })}
                />
                <input
                  className={styles.numberInput}
                  type="number"
                  min={min}
                  max={max}
                  step={0.01}
                  value={bone[field]}
                  disabled={disabled}
                  aria-label={t(`item_editor.spring_bones.${field}`)}
                  onChange={event => update(name, { [field]: clamp(Number(event.target.value) || 0, min, max) })}
                />
              </label>
            ))}
            <div className={styles.param}>
              <span>{t('item_editor.spring_bones.gravityDir')}</span>
              <div className={styles.vector} style={{ gridColumn: '1 / -1' }}>
                {AXES.map((axis, index) => (
                  <label key={axis}>
                    {axis.toUpperCase()}
                    <input
                      className={styles.numberInput}
                      type="number"
                      min={RANGES.gravityDir[0]}
                      max={RANGES.gravityDir[1]}
                      step={0.1}
                      value={bone.gravityDir[index]}
                      disabled={disabled}
                      aria-label={`${t('item_editor.spring_bones.gravityDir')} ${axis.toUpperCase()}`}
                      data-testid={`${testId}-${name}-gravity-${axis}`}
                      onChange={event => {
                        const next = [...bone.gravityDir] as [number, number, number]
                        next[index] = clamp(Number(event.target.value) || 0, RANGES.gravityDir[0], RANGES.gravityDir[1])
                        update(name, { gravityDir: next })
                      }}
                    />
                  </label>
                ))}
              </div>
            </div>
            <div className={styles.param}>
              <span>{t('item_editor.spring_bones.center')}</span>
              <div style={{ gridColumn: '1 / -1' }}>
                <BoneTreePicker
                  tree={boneTree}
                  value={bone.center}
                  disabledType="spring"
                  disabled={disabled}
                  noneLabel={t('item_editor.spring_bones.center_none')}
                  clearLabel={t('item_editor.spring_bones.center_clear')}
                  ariaLabel={t('item_editor.spring_bones.center')}
                  testId={`${testId}-${name}-center`}
                  onChange={center => update(name, { center })}
                />
              </div>
            </div>
          </div>
        )
      })}
      {!disabled && addable.length > 0 && (
        <div className={styles.addRow}>
          <Select
            value={null}
            options={addable}
            placeholder={t('item_editor.spring_bones.add')}
            tone="dark"
            ariaLabel={t('item_editor.spring_bones.add')}
            testId={`${testId}-add`}
            onChange={name => onChange({ ...params, [name]: getDefaultSpringBoneParams() })}
          />
        </div>
      )}
    </div>
  )
}
