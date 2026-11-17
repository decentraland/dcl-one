import { useCallback, useEffect, useId, useMemo, useState } from 'react';
import Button from '@ui/atoms/Button';
import Checkbox from '@ui/atoms/Checkbox';
import { ExpandMoreIcon, InfoOutlinedIcon, OpenInNewIcon } from '@ui/atoms/icons-material';
import ProgressBar from '@ui/atoms/ProgressBar';
import { Select, SelectOption } from '@ui/atoms/Select';
import Slider from '@ui/atoms/Slider';
import Text from '@ui/atoms/Text';
import Toggle from '@ui/atoms/Toggle';
import Tooltip from '@ui/atoms/Tooltip';

import {
  DEFAULT_OPTIMIZE_OPTIONS,
  type OptimizeOptions,
  type TextureCategory,
  type TextureFormat,
} from '/shared/types/optimizer';
import type { Project } from '/shared/types/projects';
import { formatBytes } from '/shared/utils';
import { misc, optimizer as optimizerPreload } from '#preload';
import { useDispatch, useSelector } from '#store';
import { useSettings } from '/@/hooks/useSettings';
import { actions } from '/@/modules/store/optimizer';
import { t } from '/@/modules/store/translation/utils';

import { Modal, type ModalCloseReason } from '../Modals';
import './styles.css';

function formatDate(epochMs: number): string {
  return new Date(epochMs).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
}

const TEXTURE_CATEGORIES: TextureCategory[] = ['baseColor', 'normal', 'orm', 'emissive', 'other'];
const FORMATS: TextureFormat[] = ['png', 'jpeg', 'webp'];
const TEXTURE_SIZES = [32, 64, 128, 256, 512, 1024, 2048, 4096];

// TODO: replace with the published "Optimize models" documentation URL.
const DOCS_URL = 'https://docs.decentraland.org/creator/optimize-models';

function InfoTip({ tip }: { tip: string }) {
  return (
    <Tooltip
      label={tip}
      side="top"
      arrow
      portal
      keepInViewport
      maxWidth={300}
    >
      <InfoOutlinedIcon
        className="info-tip"
        fontSize="small"
        tabIndex={0}
        aria-label={tip}
      />
    </Tooltip>
  );
}

// Text button with a chevron that reveals a block below it, so optional detail (the tool list,
// the per-model table) reads as "more info" rather than as a setting.
function Disclosure({
  open,
  label,
  onToggle,
}: {
  open: boolean;
  label: string;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      className={open ? 'disclosure open' : 'disclosure'}
      aria-expanded={open}
      onClick={onToggle}
    >
      <ExpandMoreIcon fontSize="small" />
      {label}
    </button>
  );
}

// A max-size field. GPUs sample mipmapped textures from power-of-two dimensions, so anything
// else is padded or rescaled by the runtime anyway — the choice is a fixed list rather than a
// free number. It also keeps the value away from 0, which reaches sharp as `resize(null, 0)`
// and throws for EVERY texture, failing the whole run under a "succeeded" result.
function SizeField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  const labelId = useId();
  return (
    <div className="size-field">
      <span id={labelId}>{label}</span>
      <Select
        size="small"
        fullWidth
        aria-labelledby={labelId}
        value={String(value)}
        onChange={size => onChange(Number(size))}
      >
        {TEXTURE_SIZES.map(size => (
          <SelectOption
            key={size}
            value={String(size)}
          >
            {size}
          </SelectOption>
        ))}
      </Select>
    </div>
  );
}

// Label + info icon, for a switch or a field. `id` lets the control take the text as its name.
function LabelWithInfo({ id, text, tip }: { id?: string; text: string; tip: string }) {
  return (
    <span className="label-with-info">
      <span id={id}>{text}</span>
      <InfoTip tip={tip} />
    </span>
  );
}

function ToggleRow({
  checked,
  text,
  tip,
  onChange,
}: {
  checked: boolean;
  text: string;
  tip: string;
  onChange: (checked: boolean) => void;
}) {
  const labelId = useId();
  return (
    <label className="toggle-row">
      <Toggle
        checked={checked}
        ariaLabelledBy={labelId}
        onChange={onChange}
      />
      <LabelWithInfo
        id={labelId}
        text={text}
        tip={tip}
      />
    </label>
  );
}

export function OptimizeModal({ project }: { project?: Project | null }) {
  const dispatch = useDispatch();
  const {
    isOpen,
    acknowledged,
    tools,
    installStatus,
    scan,
    scanStatus,
    runStatus,
    revertStatus,
    progress,
    result,
    error,
  } = useSelector(state => state.optimizer);

  const [options, setOptions] = useState<OptimizeOptions>(DEFAULT_OPTIMIZE_OPTIONS);
  const [showDetails, setShowDetails] = useState(false);
  const [showTools, setShowTools] = useState(false);
  const [dontShowAgain, setDontShowAgain] = useState(false);
  const { settings, updateAppSettings } = useSettings();
  const formatLabelId = useId();
  const qualityLabelId = useId();

  const projectPath = project?.path ?? null;
  const isRunning = runStatus === 'loading';
  const isReverting = revertStatus === 'loading';

  // The consent screen lists the pinned toolchain, so fetch that list as soon as the modal opens.
  useEffect(() => {
    if (isOpen) dispatch(actions.loadTools());
  }, [isOpen, dispatch]);

  useEffect(() => {
    if (isOpen) setDontShowAgain(false);
  }, [isOpen]);

  // A creator who asked not to see the disclosure again goes straight to the scan.
  useEffect(() => {
    if (isOpen && !acknowledged && settings.optimizerConsentAcknowledged) {
      dispatch(actions.acknowledge());
    }
  }, [isOpen, acknowledged, settings.optimizerConsentAcknowledged, dispatch]);

  // Nothing is downloaded until the creator has seen the disclosure and continued.
  useEffect(() => {
    if (isOpen && acknowledged && projectPath && tools?.status === 'missing') {
      if (installStatus === 'idle') dispatch(actions.installTools(projectPath));
    }
  }, [isOpen, acknowledged, projectPath, tools?.status, installStatus, dispatch]);

  // Scan the scene only once the tools are on disk — so nothing touches the creator's files
  // before they've decided to proceed and the run can actually happen.
  useEffect(() => {
    if (isOpen && acknowledged && projectPath && tools?.status === 'ready') {
      dispatch(actions.scanProject(projectPath));
    }
  }, [isOpen, acknowledged, projectPath, tools?.status, dispatch]);

  // Stream progress for this scene while the modal is open.
  useEffect(() => {
    if (!isOpen || !projectPath) return;
    const { cleanup } = optimizerPreload.subscribeProgress(projectPath, p =>
      dispatch(actions.setProgress(p)),
    );
    return cleanup;
  }, [isOpen, projectPath, dispatch]);

  const updateMesh = useCallback(
    (patch: Partial<OptimizeOptions['mesh']>) =>
      setOptions(o => ({ ...o, mesh: { ...o.mesh, ...patch } })),
    [],
  );
  const updateTextures = useCallback(
    (patch: Partial<OptimizeOptions['textures']>) =>
      setOptions(o => ({ ...o, textures: { ...o.textures, ...patch } })),
    [],
  );
  const updateSize = useCallback(
    (category: TextureCategory, value: number) =>
      setOptions(o => ({
        ...o,
        textures: { ...o.textures, sizes: { ...o.textures.sizes, [category]: value } },
      })),
    [],
  );

  const handleClose = useCallback(() => {
    if (!isRunning && !isReverting) dispatch(actions.close());
  }, [isRunning, isReverting, dispatch]);

  // Configuring and running a job should not be lost to a stray click beside the dialog, so
  // only the X, Escape and the buttons close it.
  const handleModalClose = useCallback(
    (_event?: unknown, reason?: ModalCloseReason) => {
      if (reason === 'backdropClick') return;
      handleClose();
    },
    [handleClose],
  );

  const handleAcknowledge = useCallback(() => {
    if (dontShowAgain) updateAppSettings({ ...settings, optimizerConsentAcknowledged: true });
    dispatch(actions.acknowledge());
  }, [dontShowAgain, settings, updateAppSettings, dispatch]);

  const handleRun = useCallback(() => {
    if (projectPath) dispatch(actions.runOptimize({ path: projectPath, options }));
  }, [projectPath, options, dispatch]);

  const handleRevert = useCallback(() => {
    if (projectPath) dispatch(actions.revertProject(projectPath));
  }, [projectPath, dispatch]);

  const percent = useMemo(() => {
    if (progress && progress.total > 0)
      return Math.round((progress.current / progress.total) * 100);
    return 0;
  }, [progress]);

  const savings = useMemo(() => {
    if (!result || result.bytesBefore === 0) return { saved: 0, percent: '0.0' };
    const saved = result.bytesBefore - result.bytesAfter;
    return { saved, percent: ((saved / result.bytesBefore) * 100).toFixed(1) };
  }, [result]);

  const details = useMemo(() => {
    const files = result?.files ?? [];
    const counts = { optimized: 0, unchanged: 0, skipped: 0, up_to_date: 0, failed: 0 };
    for (const f of files) counts[f.status]++;
    const saved = (f: (typeof files)[number]) => f.bytesBefore - f.bytesAfter;
    // Failures first (the only rows that need acting on), then most-impactful — what a creator
    // wants to scan.
    const sorted = [...files].sort((a, b) => {
      if ((a.status === 'failed') !== (b.status === 'failed')) {
        return a.status === 'failed' ? -1 : 1;
      }
      return saved(b) - saved(a);
    });
    return { counts, sorted };
  }, [result]);

  if (!isOpen || !project) return null;

  return (
    <Modal
      open={isOpen}
      size="small"
      title={t('optimize.title')}
      onClose={handleModalClose}
    >
      {!acknowledged ? (
        <div className="OptimizeModal consent">
          <Text
            variant="body2"
            className="subtitle"
          >
            {t('optimize.consent.blurb')}
          </Text>
          <Disclosure
            open={showTools}
            label={
              showTools
                ? t('optimize.consent.hide_tools')
                : t('optimize.consent.show_tools', { count: tools?.tools.length ?? 0 })
            }
            onToggle={() => setShowTools(v => !v)}
          />
          {showTools && (
            <ul className="tool-list">
              {(tools?.tools ?? []).map(tool => (
                <li key={tool.pkg}>
                  <div className="tool-head">
                    <span className="tool-name">
                      {tool.name} <span className="tool-version">v{tool.version}</span>
                    </span>
                    <span className="tool-links">
                      <button
                        type="button"
                        className="docs-link"
                        onClick={() => misc.openExternal(tool.npm)}
                      >
                        {t('optimize.consent.npm')}
                        <OpenInNewIcon fontSize="inherit" />
                      </button>
                      <button
                        type="button"
                        className="docs-link"
                        onClick={() => misc.openExternal(tool.source)}
                      >
                        {t('optimize.consent.source')}
                        <OpenInNewIcon fontSize="inherit" />
                      </button>
                    </span>
                  </div>
                  <span className="tool-purpose">
                    {t(`optimize.consent.purpose.${tool.purposeKey}`)}
                  </span>
                </li>
              ))}
            </ul>
          )}
          <Text
            variant="caption"
            className="consent-note"
          >
            {tools?.status === 'missing'
              ? `${t('optimize.consent.download', { size: tools.downloadSizeMb })} ${t('optimize.consent.note')}`
              : t('optimize.consent.note')}
          </Text>
          <Checkbox
            className="dont-show-again"
            size="sm"
            checked={dontShowAgain}
            onChange={setDontShowAgain}
          >
            {t('optimize.consent.dont_show_again')}
          </Checkbox>
          <div className="actions">
            <Button
              variant="primary"
              disabled={!tools}
              onClick={handleAcknowledge}
            >
              {t('optimize.consent.continue')}
            </Button>
            <Button
              variant="outline"
              onClick={handleClose}
            >
              {t('optimize.consent.cancel')}
            </Button>
          </div>
        </div>
      ) : tools?.status !== 'ready' ? (
        <div className="OptimizeModal installing">
          <Text
            variant="body2"
            className="subtitle"
          >
            {t('optimize.tools.installing_blurb')}
          </Text>
          {installStatus === 'failed' ? (
            <>
              <Text className="error">{t('optimize.tools.failed', { message: error ?? '' })}</Text>
              <div className="actions">
                <Button
                  variant="primary"
                  onClick={() => projectPath && dispatch(actions.installTools(projectPath))}
                >
                  {t('optimize.tools.retry')}
                </Button>
                <Button
                  variant="outline"
                  onClick={handleClose}
                >
                  {t('optimize.consent.cancel')}
                </Button>
              </div>
            </>
          ) : (
            <div className="progress">
              <ProgressBar
                indeterminate
                aria-label={t('optimize.tools.installing')}
              />
              <span className="progress-message">
                {progress?.phase === 'install' ? progress.message : t('optimize.tools.installing')}
              </span>
            </div>
          )}
        </div>
      ) : (
        <div className="OptimizeModal">
          <Text
            variant="body2"
            className="subtitle"
          >
            {t('optimize.subtitle')}
          </Text>

          <button
            type="button"
            className="docs-link"
            onClick={() => misc.openExternal(DOCS_URL)}
          >
            {t('optimize.docs_link')}
            <OpenInNewIcon fontSize="inherit" />
          </button>

          {scanStatus === 'succeeded' && scan && (
            <div className="summary">
              {scan.glbCount === 0 ? (
                <Text className="empty">{t('optimize.scan.empty')}</Text>
              ) : (
                <>
                  <span>{t('optimize.scan.glbs', { count: scan.glbCount })}</span>
                  <span>
                    {t('optimize.scan.size', {
                      size: formatBytes(scan.totalBytes),
                      models: formatBytes(scan.glbBytes),
                      textures: formatBytes(scan.textureBytes),
                    })}
                  </span>
                  <span>
                    {t('optimize.scan.textures', {
                      embedded: scan.embeddedTextureCount,
                      external: scan.externalTextureCount,
                    })}
                  </span>
                  {scan.lastOptimizedAt !== null && (
                    <span className="last-optimized">
                      {t('optimize.scan.last_optimized', {
                        date: formatDate(scan.lastOptimizedAt),
                      })}
                    </span>
                  )}
                </>
              )}
            </div>
          )}

          <div className="options">
            <Text variant="h6">{t('optimize.options.mesh.title')}</Text>
            <ToggleRow
              checked={options.mesh.enabled}
              text={t('optimize.options.mesh.enabled')}
              tip={t('optimize.options.mesh.enabled_tip')}
              onChange={enabled => updateMesh({ enabled })}
            />
            <Text variant="h6">{t('optimize.options.textures.title')}</Text>
            <ToggleRow
              checked={options.textures.compress}
              text={t('optimize.options.textures.compress')}
              tip={t('optimize.options.textures.compress_tip')}
              onChange={compress => updateTextures({ compress })}
            />
            <ToggleRow
              checked={options.textures.dedup}
              text={t('optimize.options.textures.dedup')}
              tip={t('optimize.options.textures.dedup_tip')}
              onChange={dedup => updateTextures({ dedup })}
            />
            <ToggleRow
              checked={options.textures.externalize}
              text={t('optimize.options.textures.externalize')}
              tip={t('optimize.options.textures.externalize_tip')}
              onChange={externalize => updateTextures({ externalize })}
            />

            {options.textures.compress && (
              <>
                <div className="row">
                  <LabelWithInfo
                    id={formatLabelId}
                    text={t('optimize.options.textures.format')}
                    tip={t('optimize.options.textures.format_tip')}
                  />
                  <Select
                    size="small"
                    aria-labelledby={formatLabelId}
                    value={options.textures.format}
                    onChange={format => updateTextures({ format })}
                  >
                    {FORMATS.map(f => (
                      <SelectOption
                        key={f}
                        value={f}
                      >
                        {f.toUpperCase()}
                      </SelectOption>
                    ))}
                  </Select>
                </div>

                {options.textures.format !== 'png' && (
                  <div className="quality-field">
                    <LabelWithInfo
                      id={qualityLabelId}
                      text={t('optimize.options.textures.quality')}
                      tip={t('optimize.options.textures.quality_tip')}
                    />
                    <Slider
                      min={1}
                      max={100}
                      value={options.textures.quality}
                      ariaLabelledBy={qualityLabelId}
                      onChange={quality => updateTextures({ quality })}
                    />
                  </div>
                )}

                <Text className="sizes-title">
                  <LabelWithInfo
                    text={t('optimize.options.textures.sizes')}
                    tip={t('optimize.options.textures.sizes_tip')}
                  />
                </Text>
                <div className="sizes">
                  {TEXTURE_CATEGORIES.map(cat => (
                    <SizeField
                      key={cat}
                      label={t(`optimize.options.textures.${cat}`)}
                      value={options.textures.sizes[cat]}
                      onChange={size => updateSize(cat, size)}
                    />
                  ))}
                </div>
              </>
            )}
          </div>

          {isRunning && (
            <div className="progress">
              <ProgressBar
                indeterminate={!(progress && progress.total > 0)}
                value={percent}
                aria-label={t('optimize.running')}
              />
              <span className="progress-message">{progress?.message ?? t('optimize.running')}</span>
            </div>
          )}

          {error && (runStatus === 'failed' || revertStatus === 'failed') && (
            <Text className="error">
              {/* `error` is shared, and whichever action failed most recently owns it: both
                  pending reducers clear it, and starting a run resets revertStatus. */}
              {revertStatus === 'failed'
                ? t('optimize.revert_error', { message: error })
                : t('optimize.error', { message: error })}
            </Text>
          )}

          {result && runStatus === 'succeeded' && (
            <div className="result">
              <Text variant="h6">{t('optimize.result.title')}</Text>
              <span className="saved">
                {t('optimize.result.saved', {
                  saved: formatBytes(savings.saved),
                  percent: savings.percent,
                })}
              </span>
              <span>
                {t('optimize.result.before_after', {
                  before: formatBytes(result.bytesBefore),
                  after: formatBytes(result.bytesAfter),
                })}
              </span>
              <span>{t('optimize.result.glbs_changed', { count: result.glbsChanged })}</span>
              <span>
                {t('optimize.result.textures_extracted', { count: result.texturesExtracted })}
              </span>
              <span>
                {t('optimize.result.textures_deduped', { count: result.texturesDeduped })}
              </span>
              {result.texturesRemoved > 0 && (
                <span>
                  {t('optimize.result.textures_removed', {
                    count: result.texturesRemoved,
                    bytes: formatBytes(result.removedBytes),
                  })}
                </span>
              )}
              {details.counts.up_to_date > 0 && (
                <span className="muted">
                  {t('optimize.result.up_to_date', { count: details.counts.up_to_date })}
                </span>
              )}
              {(details.counts.unchanged > 0 || details.counts.skipped > 0) && (
                <span className="muted">
                  {t('optimize.result.unchanged_skipped', {
                    unchanged: details.counts.unchanged,
                    skipped: details.counts.skipped,
                  })}
                </span>
              )}
              {details.counts.failed > 0 && (
                <span className="failed">
                  {t('optimize.result.failed', { count: details.counts.failed })}
                </span>
              )}
              {result.ignoredFiles.length > 0 && (
                <span
                  className="warning"
                  title={result.ignoredFiles.join('\n')}
                >
                  {t('optimize.result.ignored_files', {
                    count: result.ignoredFiles.length,
                    files: result.ignoredFiles.slice(0, 3).join(', '),
                  })}
                </span>
              )}

              <Disclosure
                open={showDetails}
                label={t(
                  showDetails ? 'optimize.result.hide_details' : 'optimize.result.show_details',
                )}
                onToggle={() => setShowDetails(v => !v)}
              />

              {showDetails && (
                <div className="file-details">
                  {details.sorted.map(f => {
                    const saved = f.bytesBefore - f.bytesAfter;
                    const pct =
                      f.bytesBefore > 0 ? ((saved / f.bytesBefore) * 100).toFixed(0) : '0';
                    return (
                      <div
                        className="file-row"
                        key={f.file}
                      >
                        <span
                          className="file-name"
                          title={f.file}
                        >
                          {f.file}
                        </span>
                        {f.status === 'optimized' ? (
                          <span className="file-size">
                            {formatBytes(f.bytesBefore)} → {formatBytes(f.bytesAfter)}
                            <em className="file-pct"> −{pct}%</em>
                          </span>
                        ) : (
                          <span
                            className={f.status === 'failed' ? 'file-status failed' : 'file-status'}
                            title={f.error}
                          >
                            {t(`optimize.result.status.${f.status}`)}
                          </span>
                        )}
                        <span className="file-badges">
                          {f.texturesExtracted > 0 && <em>+{f.texturesExtracted} tex</em>}
                          {f.texturesDeduped > 0 && <em>−{f.texturesDeduped} dup</em>}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          )}

          <div className="actions">
            {result && runStatus === 'succeeded' ? (
              <>
                <Button
                  variant="primary"
                  onClick={handleClose}
                >
                  {t('optimize.done')}
                </Button>
                <Button
                  variant="text"
                  disabled={!projectPath}
                  onClick={handleRun}
                >
                  {t('optimize.run_again')}
                </Button>
              </>
            ) : (
              <Button
                variant="primary"
                disabled={isRunning || !projectPath || scan?.glbCount === 0}
                onClick={handleRun}
              >
                {isRunning ? t('optimize.running') : t('optimize.run')}
              </Button>
            )}
            {scan?.hasBackup && (
              <Button
                variant="outline"
                disabled={isRunning || isReverting}
                onClick={handleRevert}
              >
                {isReverting ? t('optimize.reverting') : t('optimize.revert')}
              </Button>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}
