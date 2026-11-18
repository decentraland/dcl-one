import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router'
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels'
import { BodyShape } from '@dcl/schemas'
import {
  CheckIcon,
  CloseIcon,
  DesktopWindowsOutlinedIcon as DesktopIcon,
  InfoOutlinedIcon as InfoIcon,
  LibraryAddOutlinedIcon as AddToCollectionIcon,
  RefreshIcon,
  WarningAmberOutlinedIcon as WarningIcon
} from '@/features/creators/components/Icons'
import { AvatarPreview } from '@/features/creators/components/AvatarPreview'
import { Button } from '@/features/creators/components/Button'
import { CategorySelect } from '@/features/creators/components/CategorySelect'
import { type AddItemsPrefill } from '@/features/creators/components/CollectionDetailPage/AddItemsModal'
import { AvatarCustomizerDrawer, AvatarCustomizerToggle } from '@/features/creators/components/ItemEditorPage/AvatarCustomizer'
import { EditorSection } from '@/features/creators/components/ItemEditorPage/EditorSection'
import { HidesEditor } from '@/features/creators/components/ItemEditorPage/HidesEditor'
import { PlaybackBar } from '@/features/creators/components/ItemEditorPage/PlaybackBar'
import { SceneBoundariesToggle } from '@/features/creators/components/ItemEditorPage/SceneBoundariesToggle'
import { SpringBonesEditor } from '@/features/creators/components/ItemEditorPage/SpringBonesEditor'
import { ValidationBadge, getValidationStatus } from '@/features/creators/components/ValidationBadge'
import { Switch } from '@/features/creators/components/Switch'
import { ZoomControls } from '@/features/creators/components/ZoomControls'
import { useBaseWearables } from '@/features/creators/hooks/useBaseWearables'
import { useLiveBridge } from '@/features/creators/hooks/useLiveBridge'
import { useMediaQuery } from '@/features/creators/hooks/useMediaQuery'
import { useModelValidation } from '@/features/creators/hooks/useModelValidation'
import { type SpringBonesModel } from '@/features/creators/hooks/useSpringBones'
import { useIntl, useTranslation } from '@/data/lib/creators/intl'
import { track } from '@/data/lib/creators/analytics'
import { type AvatarAttributes } from '@/data/lib/creators/avatar'
import { createPanelStorage } from '@/data/lib/creators/itemEditor'
import { EmotePlayMode } from '@/data/lib/creators/itemFactory'
import { ItemType } from '@/data/lib/creators/items'
import { MODEL_KEY, buildDefinition, isEmoteState, resolveBridgeUrl } from '@/data/lib/creators/livePreview'
import { captureError } from '@/data/lib/creators/monitoring'
import { type AvatarPreviewSource } from '@/data/lib/creators/preview'
import {
  getDefaultSpringBoneRoots,
  hasSpringBones,
  parseSpringBones,
  type BoneNode,
  type SpringBoneParamsByName
} from '@/data/lib/creators/springBones'
import { formatTimeAgo } from '@/data/lib/creators/time'
import { getWearableCategoryOptions } from '@/data/lib/creators/wearableCategories'
import { selectAvatarAttributes, useAvatarPreview } from '@/features/creators/store/avatarPreview'
import { useWallet } from '@/features/creators/store/wallet'
import { media } from '@/data/lib/creators/theme'
import { AddToCollectionFlow } from './AddToCollectionFlow'
import styles from './LivePreviewPage.module.css'
import editor from '../ItemEditorPage/ItemEditorPage.module.css'
import { type WearablePreviewController } from '@/features/creators/components/WearablePreview'

const PREVIEW_ID = 'live-preview'
// The streamed model has no storage hash; the spring bones editor just needs a stable key.
const SPRING_HASH = 'live'
const SPRING_BONES_PUSH_DELAY_MS = 500
const TIME_AGO_TICK_MS = 30_000
const JUST_NOW_MS = 60_000
// How long the label acknowledges a refresh that found nothing new.
const UP_TO_DATE_MS = 4_000
const BOTH_SHAPES = [BodyShape.MALE, BodyShape.FEMALE]
// Panel sizes as react-resizable-panels wants them, percentages of the group.
const LEFT_DEFAULT_PCT = 25
const LEFT_MIN_PCT = 18
const CENTER_MIN_PCT = 25

/**
 * How old the model on screen is. Background polling never touches it; a refresh that found nothing
 * new briefly turns it into an "up to date" acknowledgement. Isolated so the periodic tick keeping
 * the time fresh doesn't re-render the page.
 */
function UpdatedLabel({ timestamp, checkedAt }: { timestamp: number; checkedAt: number | null }) {
  const { t } = useTranslation()
  const { locale } = useIntl()
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const interval = setInterval(() => setNow(Date.now()), TIME_AGO_TICK_MS)
    return () => clearInterval(interval)
  }, [])
  const [upToDate, setUpToDate] = useState(false)
  useEffect(() => {
    if (checkedAt === null) return
    setUpToDate(true)
    const timer = setTimeout(() => setUpToDate(false), UP_TO_DATE_MS)
    return () => clearTimeout(timer)
  }, [checkedAt])
  const timeAgo = now - timestamp < JUST_NOW_MS ? t('live_preview.updated_now') : formatTimeAgo(timestamp, locale, now)
  return (
    <span
      className={styles.meta}
      data-testid="live-preview-updated"
      data-up-to-date={upToDate || undefined}
      title={new Date(timestamp).toLocaleTimeString(locale)}
    >
      {upToDate && <CheckIcon aria-hidden />}
      {t(upToDate ? 'live_preview.up_to_date' : 'live_preview.updated', { time_ago: timeAgo })}
    </span>
  )
}

/**
 * Shown in the preview area, away from the address bar where the browser's own permission bubble
 * drops down and would cover a hint placed in the side panel.
 */
function PermissionCard({ state, onRetry }: { state: 'prompt' | 'denied'; onRetry: () => void }) {
  const { t } = useTranslation()
  const denied = state === 'denied'
  return (
    <div className={styles.permissionCard} data-state={state} data-testid="live-preview-permission">
      {denied ? <WarningIcon /> : <InfoIcon />}
      <h2>{t(`live_preview.local_network.${state}.title`)}</h2>
      <p>{t(`live_preview.local_network.${state}.description`)}</p>
      {denied && (
        <>
          <ol>
            <li>{t('live_preview.local_network.denied.step_site_info')}</li>
            <li>{t('live_preview.local_network.denied.step_allow')}</li>
            <li>{t('live_preview.local_network.denied.step_reload')}</li>
          </ol>
          <div className={styles.cardActions}>
            <Button type="button" size="sm" onClick={() => window.location.reload()}>
              {t('live_preview.local_network.denied.reload')}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              data-testid="live-preview-permission-retry"
              onClick={onRetry}
            >
              {t('live_preview.local_network.denied.retry')}
            </Button>
          </div>
        </>
      )}
    </div>
  )
}

const LivePreviewPage = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const isMobile = useMediaQuery(media.maxWidth('mobile'))
  const [bridgeUrl, setBridgeUrl] = useState(() => resolveBridgeUrl(searchParams.get('bridge')))
  const bridge = useLiveBridge(bridgeUrl)
  const { state, glb } = bridge
  const isConnected = bridge.status === 'connected' || bridge.status === 'connecting'
  const isEmote = !!state && isEmoteState(state)

  // Panel tuning. A null category follows the bridge until the creator picks one.
  const [categoryOverride, setCategoryOverride] = useState<string | null>(null)
  const [hides, setHides] = useState<string[]>([])
  const [loop, setLoop] = useState(true)
  const category = categoryOverride ?? state?.category ?? null

  // Avatar (session store), as in the item editor.
  const bodyShape = useAvatarPreview(s => s.bodyShape)
  const skin = useAvatarPreview(s => s.skin)
  const eyes = useAvatarPreview(s => s.eyes)
  const hair = useAvatarPreview(s => s.hair)
  const baseSelection = useAvatarPreview(s => s.baseWearables)
  const emote = useAvatarPreview(s => s.emote)
  const isPlaying = useAvatarPreview(s => s.isPlaying)
  const seedBaseWearables = useAvatarPreview(s => s.seedBaseWearables)
  const baseWearables = useBaseWearables()
  useEffect(() => {
    if (baseWearables.data) seedBaseWearables(baseWearables.data)
  }, [baseWearables.data, seedBaseWearables])
  const avatar = useMemo<AvatarAttributes>(
    () => selectAvatarAttributes({ bodyShape, skin, eyes, hair, baseWearables: baseSelection }),
    [bodyShape, skin, eyes, hair, baseSelection]
  )

  const definition = useMemo(
    () => (state && glb ? buildDefinition(state, glb, { category: categoryOverride, hides, loop }) : null),
    [state, glb, categoryOverride, hides, loop]
  )
  const source = useMemo<AvatarPreviewSource | null>(
    () => (definition ? { kind: 'definition', definition } : null),
    [definition]
  )

  // Spring bones, parsed from each streamed GLB. Params tuned on earlier pushes survive; bones that
  // left the export are dropped, new chain roots get defaults, roots the creator removed stay removed.
  const [bones, setBones] = useState<BoneNode[]>([])
  const [springParams, setSpringParams] = useState<SpringBoneParamsByName>({})
  const removedRootsRef = useRef(new Set<string>())
  useEffect(() => {
    if (!glb) return
    let cancelled = false
    glb
      .arrayBuffer()
      .then(buffer => {
        if (cancelled) return
        const parsed = parseSpringBones(buffer)
        setBones(parsed)
        setSpringParams(previous => {
          const names = new Set(parsed.filter(bone => bone.type === 'spring').map(bone => bone.name))
          const next: SpringBoneParamsByName = {}
          for (const [name, params] of Object.entries(previous)) if (names.has(name)) next[name] = params
          for (const [name, params] of Object.entries(getDefaultSpringBoneRoots(parsed))) {
            if (!(name in next) && !removedRootsRef.current.has(name)) next[name] = params
          }
          return next
        })
      })
      .catch((error: unknown) => captureError(error, { flow: 'live_preview', step: 'parse_spring_bones' }))
    return () => {
      cancelled = true
    }
  }, [glb])
  const springModels = useMemo<SpringBonesModel[]>(
    () =>
      !isEmote && hasSpringBones(bones)
        ? [{ hash: SPRING_HASH, bodyShapes: state?.bodyShapes ?? BOTH_SHAPES, bones }]
        : [],
    [isEmote, bones, state?.bodyShapes]
  )
  const onSpringChange = useCallback((next: SpringBoneParamsByName) => {
    setSpringParams(previous => {
      for (const name of Object.keys(previous)) if (!(name in next)) removedRootsRef.current.add(name)
      for (const name of Object.keys(next)) removedRootsRef.current.delete(name)
      return next
    })
  }, [])

  // Preview controller: spring bones are pushed on edits (debounced), on every load and on play.
  const [controller, setController] = useState<WearablePreviewController | null>(null)
  const [loadCount, setLoadCount] = useState(0)
  const onPreviewLoad = useCallback(() => setLoadCount(count => count + 1), [])
  const onPreviewError = useCallback(
    (error: Error) => captureError(error, { flow: 'live_preview', step: 'preview' }),
    []
  )
  const definitionId = definition?.id ?? null
  const hasSprings = springModels.length > 0
  const physicsReportedRef = useRef(false)
  const pushSpringBones = useCallback(() => {
    if (!controller || !definitionId || !hasSprings) return
    controller.physics.setSpringBonesParams(definitionId, springParams).catch((error: unknown) => {
      if (physicsReportedRef.current) return
      physicsReportedRef.current = true
      captureError(error, { flow: 'live_preview', step: 'physics' })
    })
  }, [controller, definitionId, hasSprings, springParams])
  const pushSpringBonesRef = useRef(pushSpringBones)
  pushSpringBonesRef.current = pushSpringBones
  useEffect(() => {
    const timer = window.setTimeout(pushSpringBones, SPRING_BONES_PUSH_DELAY_MS)
    return () => window.clearTimeout(timer)
  }, [pushSpringBones])
  useEffect(() => {
    if (loadCount > 0 || isPlaying) pushSpringBonesRef.current()
  }, [loadCount, isPlaying])

  // Validation of the streamed model, re-run on every push and on category / hides edits.
  const validationSource = useMemo(
    () => (glb ? ({ kind: 'blob', contents: { [MODEL_KEY]: glb }, mainFile: MODEL_KEY } as const) : null),
    [glb]
  )
  const validationCtx = useMemo(
    () => ({
      type: isEmote ? ItemType.EMOTE : ItemType.WEARABLE,
      category: category ?? undefined,
      hides: isEmote ? undefined : hides
    }),
    [isEmote, category, hides]
  )
  const validation = useModelValidation(validationSource, validationCtx, definitionId ?? undefined)
  const validationStatus = useMemo(
    () => getValidationStatus(validation.data?.issues, validation.isLoading),
    [validation.data?.issues, validation.isLoading]
  )

  // One tune event per control per streamed version: the funnel wants "did they tune", not every drag.
  const tunedRef = useRef(new Set<string>())
  const trackTune = useCallback(
    (control: 'category' | 'hides' | 'loop' | 'spring_bones') => {
      const key = `${String(state?.version)}:${control}`
      if (tunedRef.current.has(key)) return
      tunedRef.current.add(key)
      track('Live preview tune', { control, item_type: isEmote ? 'emote' : 'wearable' })
    },
    [state?.version, isEmote]
  )

  // Add to collection: the model is snapshotted so pushes while the modals are open don't affect it.
  const [adding, setAdding] = useState<{ file: File; prefill: AddItemsPrefill } | null>(null)
  const session = useWallet(s => s.session)
  const addToCollection = useCallback(() => {
    if (!glb || !state) return
    track('Live preview add to collection', {
      item_type: isEmote ? 'emote' : 'wearable',
      push_count: bridge.pushCount,
      signed_in: !!session
    })
    setAdding({
      file: new File([glb], MODEL_KEY, { type: 'model/gltf-binary' }),
      prefill: isEmote
        ? { playMode: loop ? EmotePlayMode.LOOP : EmotePlayMode.SIMPLE }
        : {
            category: category ?? undefined,
            hides,
            springBoneParams: hasSprings && Object.keys(springParams).length > 0 ? springParams : undefined
          }
    })
  }, [glb, state, isEmote, bridge.pushCount, session, loop, category, hides, hasSprings, springParams])

  const [isCustomizerOpen, setCustomizerOpen] = useState(false)
  const closeCustomizer = useCallback(() => setCustomizerOpen(false), [])
  const panelStorage = useMemo(() => createPanelStorage(), [])
  const contents = useMemo(() => ({ [MODEL_KEY]: glb ?? new Blob() }), [glb])
  const categories = useMemo(() => getWearableCategoryOptions(contents), [contents])
  const permission = bridge.permission === 'prompt' || bridge.permission === 'denied' ? bridge.permission : null

  const preview = !source ? null : (
    <AvatarPreview
      id={PREVIEW_ID}
      source={source}
      avatar={avatar}
      emote={emote}
      onLoad={onPreviewLoad}
      onError={onPreviewError}
      onController={setController}
    >
      <PlaybackBar
        previewId={PREVIEW_ID}
        controller={controller}
        collectionEmotes={[]}
        previewedWearables={[]}
        subjectEmoteId={isEmote ? definitionId : null}
      />
      <AvatarCustomizerToggle open={isCustomizerOpen} onToggle={() => setCustomizerOpen(open => !open)} />
      <SceneBoundariesToggle trigger="live_preview" />
      <ValidationBadge
        status={validationStatus}
        issues={validation.data?.issues ?? []}
        subject={{
          name: t('item_editor.validation.previewed_item'),
          type: validationCtx.type,
          category: validationCtx.category
        }}
      />
    </AvatarPreview>
  )

  const placeholder = permission ? (
    <PermissionCard state={permission} onRetry={bridge.connect} />
  ) : bridge.status === 'connecting' ? (
    <span className="spinner" aria-hidden />
  ) : (
    <span data-testid="live-preview-waiting">{t('live_preview.waiting')}</span>
  )

  const center = (
    <>
      <div className={editor.previewArea}>
        {preview ?? (
          <div className={editor.previewEmpty} data-testid="live-preview-empty">
            {placeholder}
          </div>
        )}
        {preview && loadCount > 0 && controller && <ZoomControls controller={controller} />}
      </div>
      {isCustomizerOpen && <AvatarCustomizerDrawer catalog={baseWearables.data} onClose={closeCustomizer} />}
    </>
  )

  const modals = adding && (
    <AddToCollectionFlow
      file={adding.file}
      prefill={adding.prefill}
      onClose={() => setAdding(null)}
      onDone={collectionId => navigate(`/create/collections/${collectionId}`)}
    />
  )

  if (isMobile) {
    return (
      <div className={editor.mobileWorkspace} data-testid="live-preview-mobile">
        <div className={editor.mobilePreviewArea}>{center}</div>
        <MobileHint />
        {modals}
      </div>
    )
  }

  return (
    <div className={editor.workspace} data-testid="live-preview-page">
      <div className={editor.columns}>
        <PanelGroup
          direction="horizontal"
          autoSaveId="live-preview"
          storage={panelStorage}
          style={{ flex: 1, minWidth: 0 }}
        >
          <Panel id="live-preview-panel" defaultSize={LEFT_DEFAULT_PCT} minSize={LEFT_MIN_PCT} order={1}>
            <aside className={styles.sidePanel}>
              <div className={editor.panelBody}>
                <EditorSection
                  title={t('live_preview.title')}
                  testId="live-preview-connection"
                  adornment={
                    <span className={styles.statusPill} data-status={bridge.status} data-testid="live-preview-status">
                      {t(`live_preview.status.${bridge.status}`)}
                    </span>
                  }
                >
                  {/* A div, not the usual label: two labelable controls may not share one label. */}
                  <div className={editor.field}>
                    <span className={editor.fieldLabel}>{t('live_preview.bridge_url')}</span>
                    <div className={styles.inputRow}>
                      <input
                        className={editor.textInput}
                        value={bridgeUrl}
                        disabled={isConnected}
                        placeholder="http://localhost:8080"
                        aria-label={t('live_preview.bridge_url')}
                        data-testid="live-preview-bridge-url"
                        onChange={event => setBridgeUrl(event.target.value)}
                      />
                      {isConnected ? (
                        <Button
                          className={styles.connectButton}
                          type="button"
                          size="sm"
                          variant="secondary"
                          data-testid="live-preview-disconnect"
                          onClick={bridge.disconnect}
                        >
                          {t('live_preview.disconnect')}
                        </Button>
                      ) : (
                        <Button
                          className={styles.connectButton}
                          type="button"
                          size="sm"
                          data-testid="live-preview-connect"
                          onClick={bridge.connect}
                        >
                          {t('live_preview.connect')}
                        </Button>
                      )}
                    </div>
                  </div>
                  {isConnected && (
                    <div className={styles.row}>
                      <Button
                        type="button"
                        size="sm"
                        variant="dark"
                        loading={bridge.isRefreshing}
                        data-testid="live-preview-refresh"
                        onClick={bridge.refresh}
                      >
                        <RefreshIcon fontSize="small" />
                        {t('live_preview.refresh')}
                      </Button>
                      {bridge.lastUpdateAt !== null && (
                        <UpdatedLabel timestamp={bridge.lastUpdateAt} checkedAt={bridge.lastCheckedAt} />
                      )}
                    </div>
                  )}
                  {bridge.errorCode && (
                    <p className={styles.errorText} data-testid="live-preview-error">
                      {t(`live_preview.errors.${bridge.errorCode}`)}
                    </p>
                  )}
                </EditorSection>

                {state && isEmote && (
                  <EditorSection title={t('item_editor.animation.title')} testId="live-preview-animation">
                    <div className={editor.toggle}>
                      <span>{t('live_preview.loop')}</span>
                      <Switch
                        checked={loop}
                        label={t('live_preview.loop')}
                        testId="live-preview-loop"
                        onChange={checked => {
                          setLoop(checked)
                          trackTune('loop')
                        }}
                      />
                    </div>
                  </EditorSection>
                )}

                {state && !isEmote && (
                  <>
                    <EditorSection title={t('live_preview.properties')} testId="live-preview-properties">
                      <label className={editor.field}>
                        <span className={editor.fieldLabel}>{t('item_editor.basics.category')}</span>
                        <CategorySelect
                          value={category}
                          categories={categories}
                          tone="dark"
                          testId="live-preview-category"
                          onChange={value => {
                            setCategoryOverride(value)
                            trackTune('category')
                          }}
                        />
                      </label>
                    </EditorSection>
                    <EditorSection title={t('item_editor.overrides.title')} testId="live-preview-overrides">
                      <HidesEditor
                        contents={contents}
                        category={category}
                        hides={hides}
                        testId="live-preview-hides"
                        onChange={value => {
                          setHides(value)
                          trackTune('hides')
                        }}
                      />
                    </EditorSection>
                    {hasSprings && (
                      <EditorSection title={t('item_editor.spring_bones.title')} testId="live-preview-spring-bones">
                        <SpringBonesEditor
                          models={springModels}
                          activeHash={SPRING_HASH}
                          onActiveHashChange={() => undefined}
                          params={springParams}
                          onChange={next => {
                            onSpringChange(next)
                            trackTune('spring_bones')
                          }}
                        />
                      </EditorSection>
                    )}
                  </>
                )}

                <div className={editor.footer}>
                  <Button
                    className={styles.addButton}
                    type="button"
                    disabled={!definition}
                    data-testid="live-preview-add"
                    onClick={addToCollection}
                  >
                    <AddToCollectionIcon fontSize="small" />
                    {t('live_preview.add_to_collection')}
                  </Button>
                </div>
              </div>
            </aside>
          </Panel>
          <PanelResizeHandle>
            <div className={editor.handle} data-testid="resize-handle-left" />
          </PanelResizeHandle>
          <Panel id="live-preview-center" minSize={CENTER_MIN_PCT} order={2}>
            <div className={editor.centerPanel} data-testid="live-preview-center">
              {center}
            </div>
          </Panel>
        </PanelGroup>
      </div>
      {modals}
    </div>
  )
}

function MobileHint() {
  const { t } = useTranslation()
  const [dismissed, setDismissed] = useState(false)
  if (dismissed) return null
  return (
    <div className={editor.mobileHint} data-testid="live-preview-mobile-hint">
      <DesktopIcon fontSize="small" />
      <span>{t('live_preview.mobile_hint')}</span>
      <button type="button" aria-label={t('modal.close')} onClick={() => setDismissed(true)}>
        <CloseIcon fontSize="small" />
      </button>
    </div>
  )
}

export { LivePreviewPage }
