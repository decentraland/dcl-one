import { type CSSProperties, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  StopIcon,
  ArrowUpwardIcon,
  AttachFileIcon,
  ImageOutlinedIcon,
  ViewInArOutlinedIcon,
  AudiotrackIcon,
  InsertDriveFileOutlinedIcon,
  AddCommentOutlinedIcon,
  CheckIcon,
  KeyboardArrowDownIcon,
  CloseIcon,
  RefreshIcon,
  UndoIcon,
  HighlightAltIcon,
  ContentCopyIcon,
  OpenInNewIcon,
  DeleteOutlineIcon,
} from '@ui/atoms/icons-material';
import { cx } from '/@/lib/cx';
import Markdown, { type MarkdownToJSX } from 'markdown-to-jsx';
import Button from '@ui/atoms/Button';
import IconButton from '@ui/atoms/IconButton';
import { Menu, MenuItem, MenuSeparator } from '@ui/atoms/Menu';
import { Select, SelectOption } from '@ui/atoms/Select';
import Spinner from '@ui/atoms/Spinner';
import TextField from '@ui/atoms/TextField';
import Tooltip from '@ui/atoms/Tooltip';

import type {
  AiAssetMention,
  AiAttachment,
  AiAttachmentKind,
  AiPendingRecording,
  AiProvider,
  AiRecorderState,
} from '/shared/types/ai';
import {
  MIN_CLAUDE_CLI_VERSION,
  assetMentionKey,
  cliSetup,
  isCliVersionOutdated,
} from '/shared/types/ai';
import { formatRecordingDuration } from '/shared/types/recording';

import { ai as aiPreload, misc } from '#preload';
import { t } from '/@/modules/store/translation/utils';
import { useCliSignIn } from '/@/hooks/useCliSignIn';

import type {
  AiMessage,
  AiPromptData,
  AiSessionMeta,
  DraftPromptMode,
} from '/@/modules/store/ai/types';
import { MicrophoneIcon, WarningCircleIcon } from '../Icons';
import { RecordButton, RecordingCard, TranscribingCard } from '../SessionRecorder';
import { SignInCodeField } from '../SignInCodeField';
import { toolChipLabel } from './labels';
import recordRowStyles from './RecordRow.module.css';
import styles from './component.module.css';

// Render assistant replies as markdown. Raw HTML is disabled so nothing the model emits
// can inject markup, and links open in the default browser (the Electron security layer
// only lets allowlisted origins through, blocking the rest — no navigation of the app).
const MARKDOWN_OPTIONS: MarkdownToJSX.Options = {
  disableParsingRawHTML: true,
  overrides: { a: { props: { target: '_blank', rel: 'noopener noreferrer' } } },
};

// Compact "last used" label for a session in the history menu.
function formatWhen(ts: number): string {
  const min = Math.floor((Date.now() - ts) / 60_000);
  if (min < 1) return t('editor.ai.history.now');
  if (min < 60) return t('editor.ai.history.minutes', { n: min });
  const hr = Math.floor(min / 60);
  if (hr < 24) return t('editor.ai.history.hours', { n: hr });
  const day = Math.floor(hr / 24);
  if (day < 7) return t('editor.ai.history.days', { n: day });
  return new Date(ts).toLocaleDateString();
}

// The plain-text content of a message (its text parts joined) — for the user bubble and for
// re-sending the last prompt on retry.
function messageText(msg: AiMessage): string {
  return msg.parts.map(p => (p.kind === 'text' ? p.text : '')).join('');
}

function isAwaitingAnswer(msg: AiMessage | undefined): boolean {
  return (
    msg?.parts.some(
      p => p.kind === 'prompt' && p.prompt.answer === undefined && p.prompt.dismissed !== true,
    ) ?? false
  );
}

// Cap on how many files a single prompt carries (mirrors MAX_ATTACHMENTS in main/modules/ai.ts).
const MAX_ATTACHMENTS = 8;

const IMAGE_EXTS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'avif'];
const MODEL_EXTS = ['glb', 'gltf'];
const AUDIO_EXTS = ['mp3', 'wav', 'ogg', 'm4a', 'flac'];

// Classify a file for its chip icon and codex's image-only -i flag. Prefers the MIME type the OS
// reported, falling back to the extension (dropped files often arrive with an empty MIME type).
function classifyAttachment(name: string, mimeType: string): AiAttachmentKind {
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  if (mimeType.startsWith('image/') || IMAGE_EXTS.includes(ext)) return 'image';
  if (mimeType.startsWith('model/') || MODEL_EXTS.includes(ext)) return 'model';
  if (mimeType.startsWith('audio/') || AUDIO_EXTS.includes(ext)) return 'audio';
  return 'file';
}

// Turn a dropped/picked/pasted File into an attachment. A file on disk (drop or picker) resolves
// to its real path via preload — Electron hides File.path from the renderer, so main hands the
// CLI that path directly (no copy, no size limit). A pasted blob has no path, so read its bytes
// into a data URL and let main temp-file it.
async function fileToAttachment(file: File): Promise<AiAttachment | null> {
  const kind = classifyAttachment(file.name, file.type);
  const mimeType = file.type !== '' ? file.type : undefined;
  const name = file.name !== '' ? file.name : 'attachment';
  const diskPath = aiPreload.getPathForFile(file);
  if (diskPath !== '') return { name, kind, mimeType, path: diskPath };
  const dataUrl = await new Promise<string | null>(resolve => {
    const reader = new FileReader();
    reader.onload = () => resolve(typeof reader.result === 'string' ? reader.result : null);
    reader.onerror = () => resolve(null);
    reader.readAsDataURL(file);
  });
  return dataUrl === null ? null : { name, kind, mimeType, dataUrl };
}

// Whether a drag carries files (not a text/selection drag within the composer).
function isFileDrag(e: React.DragEvent): boolean {
  return Array.from(e.dataTransfer.types).includes('Files');
}

function AttachmentIcon({ kind }: { kind: AiAttachmentKind }) {
  switch (kind) {
    case 'image':
      return <ImageOutlinedIcon />;
    case 'model':
      return <ViewInArOutlinedIcon />;
    case 'audio':
      return <AudiotrackIcon />;
    default:
      return <InsertDriveFileOutlinedIcon />;
  }
}

// An interactive `ask_user` prompt rendered inline in the transcript. Single-select answers on
// click; multi-select toggles then confirms; free-text (allowOther / no options) uses the field.
function PromptBlock({
  prompt,
  onAnswer,
}: {
  prompt: AiPromptData;
  onAnswer: (answer: string) => void;
}) {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [other, setOther] = useState('');
  const answered = prompt.answer !== undefined;
  const disabled = answered || prompt.dismissed === true;
  const showOther = prompt.allowOther || prompt.options.length === 0;

  const toggle = (label: string) =>
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(label)) next.delete(label);
      else next.add(label);
      return next;
    });

  const submitMulti = () => {
    const picks = prompt.options.filter(o => selected.has(o.label)).map(o => o.label);
    if (other.trim() !== '') picks.push(other.trim());
    if (picks.length > 0) onAnswer(picks.join(', '));
  };
  const submitOther = () => {
    if (other.trim() !== '') onAnswer(other.trim());
  };

  return (
    <div className={styles.promptBox}>
      <div className={styles.promptQuestion}>{prompt.question}</div>
      {answered ? (
        <div className={styles.promptAnswer}>{prompt.answer}</div>
      ) : (
        <>
          {prompt.options.length > 0 && (
            <div className={styles.promptOptions}>
              {prompt.options.map(o => (
                <button
                  className={styles.promptOption}
                  key={o.label}
                  type="button"
                  disabled={disabled}
                  data-selected={(prompt.multiSelect && selected.has(o.label)) || undefined}
                  onClick={() => {
                    if (disabled) return;
                    if (prompt.multiSelect) toggle(o.label);
                    else onAnswer(o.label);
                  }}
                >
                  <span>{o.label}</span>
                  {o.description !== undefined && o.description !== '' && (
                    <span className={styles.promptOptionDesc}>{o.description}</span>
                  )}
                </button>
              ))}
            </div>
          )}
          {showOther && (
            <div className={styles.promptOtherRow}>
              <TextField
                fullWidth
                multiline
                maxRows={4}
                size="small"
                className={styles.promptField}
                placeholder={t('editor.ai.prompt.other_placeholder')}
                value={other}
                disabled={disabled}
                onChange={e => setOther(e.target.value)}
                onKeyDown={e => {
                  if (e.key === 'Enter' && !e.shiftKey && !prompt.multiSelect) {
                    e.preventDefault();
                    submitOther();
                  }
                }}
              />
              {!prompt.multiSelect && (
                <IconButton
                  className={styles.sendButton}
                  aria-label={t('editor.ai.send')}
                  disabled={disabled || other.trim() === ''}
                  onClick={submitOther}
                >
                  <ArrowUpwardIcon fontSize="small" />
                </IconButton>
              )}
            </div>
          )}
          {prompt.multiSelect && (
            <Button
              variant="text"
              size="sm"
              className={styles.textButton}
              disabled={disabled || (selected.size === 0 && other.trim() === '')}
              onClick={submitMulti}
            >
              {t('editor.ai.prompt.confirm')}
            </Button>
          )}
          {prompt.dismissed === true && (
            <span className={styles.promptNote}>{t('editor.ai.prompt.dismissed')}</span>
          )}
        </>
      )}
    </div>
  );
}

// The pure chat surface, driven entirely by props. Both the inline panel (redux-backed)
// and the detached window (mirror-backed) render this — neither reaches into a store from
// here, so the same view works whichever owns the state (#1504).
export interface ChatViewProps {
  providers: {
    id: AiProvider;
    label: string;
    available: boolean;
    reason?: string;
    version?: string;
    managedSignIn: boolean;
    desktopInstalled?: boolean;
    needsLogin?: boolean;
  }[];
  provider: AiProvider;
  messages: AiMessage[];
  busy: boolean;
  detecting: boolean;
  selection: { id: number; name: string }[];
  // Local asset paths mentioned for the next prompt (double-clicked in the Local Assets panel).
  assetMentions: AiAssetMention[];
  /** A finished session recording that goes out with the next message. */
  recording: AiPendingRecording | null;
  /** The Record button's state; with `onToggleRecording` it is shown in the composer. */
  recorder?: AiRecorderState;
  onToggleRecording?: () => void;
  /** Discard the running recording; the card asks for confirmation first. */
  onCancelRecording?: () => void;
  /** Send the recording without waiting for its transcript; shown while it transcribes. */
  onSkipTranscription?: () => void;
  // The user dismissed the billing hint for this scene (#1505) — hide it.
  billingDismissed: boolean;
  // The scene's saved conversations (newest first) and which one is active, for the history menu.
  sessions: AiSessionMeta[];
  currentSessionId: string;
  // Shown after the title in the header (the open project) — the detached window uses it.
  title?: string;
  // Inline panel width in px (the user-draggable size). Ignored when detached (fills the window).
  width?: number;
  // True when rendered as the detached window: fills the window and shows a "dock" affordance
  // instead of "pop out".
  detached?: boolean;
  onSend: (text: string, attachments?: AiAttachment[]) => void;
  onStop: () => void;
  onNewChat: () => void;
  onProviderChange: (provider: AiProvider) => void;
  onRevertTurn: (id: string, count: number) => void;
  // Answer an interactive `ask_user` prompt (unblocks the agent's waiting tool call).
  onAnswerPrompt: (id: string, answer: string) => void;
  onRecheck: () => void;
  // Dismiss the billing hint for this scene (persisted per-project).
  onDismissBilling: () => void;
  // Open a past session from the history / delete one.
  onSwitchSession: (id: string) => void;
  onDeleteSession: (id: string) => void;
  // Deselect all entities (clears the selection chips).
  onClearSelection: () => void;
  // Drop one mentioned asset from the next prompt.
  onRemoveAssetMention: (mention: AiAssetMention) => void;
  /** Discard the pending recording instead of sending it. */
  onRemoveRecording: () => void;
  // Open the detached window (inline only — omitted/no-op in the detached window).
  onPopOut?: () => void;
  // Inline: hide the panel. Detached: dock the chat back inline (close the window).
  onClose: () => void;
  // A prompt seeded from the inspector (Trigger Area "describe a reaction"). When its nonce
  // changes the composer copies `text` into its input and calls onDraftConsumed. Inline only.
  draftPrompt?: { text: string; nonce: number; mode: DraftPromptMode } | null;
  onDraftConsumed?: () => void;
}

/** What the composer's chip says about everything the next message will carry. */
function recordingChipLabel(recording: AiPendingRecording): string {
  const values = {
    count: recording.sessions.length,
    duration: formatRecordingDuration(recording.durationMs),
    actions: recording.actionCount,
  };
  const key =
    recording.sessions.length > 1
      ? recording.transcriptionError
        ? 'editor.ai.recording.chip_many_no_speech'
        : 'editor.ai.recording.chip_many'
      : recording.transcriptionError
        ? 'editor.ai.recording.chip_no_speech'
        : 'editor.ai.recording.chip';
  return t(key, values);
}

function ClearMentionButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <Tooltip
      label={label}
      side="bottom"
      portal
      keepInViewport
    >
      <IconButton
        size="small"
        aria-label={label}
        className={styles.clearMention}
        onClick={onClick}
      >
        <CloseIcon className={styles.clearMentionIcon} />
      </IconButton>
    </Tooltip>
  );
}

export function ChatView(props: ChatViewProps) {
  const {
    providers,
    provider,
    messages,
    busy,
    detecting,
    selection,
    assetMentions,
    recording,
    recorder,
    onToggleRecording,
    onCancelRecording,
    onSkipTranscription,
    billingDismissed,
    sessions,
    currentSessionId,
    title,
    width,
    detached = false,
    onSend,
    onStop,
    onNewChat,
    onProviderChange,
    onRevertTurn,
    onAnswerPrompt,
    onRecheck,
    onDismissBilling,
    onSwitchSession,
    onDeleteSession,
    onClearSelection,
    onRemoveAssetMention,
    onRemoveRecording,
    onPopOut,
    onClose,
    draftPrompt,
    onDraftConsumed,
  } = props;

  const [input, setInput] = useState('');
  const appliedDraftNonce = useRef(0);
  const wasRecording = useRef(recorder?.status === 'recording');
  const recordingJustStarted = recorder?.status === 'recording' && !wasRecording.current;
  useEffect(() => {
    wasRecording.current = recorder?.status === 'recording';
  }, [recorder?.status]);
  const [attachments, setAttachments] = useState<AiAttachment[]>([]);
  const [dragging, setDragging] = useState(false);
  const transcriptRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!draftPrompt || draftPrompt.nonce === appliedDraftNonce.current) return;
    appliedDraftNonce.current = draftPrompt.nonce;
    const append = draftPrompt.mode === 'append';
    const wasInChat = panelRef.current?.contains(document.activeElement) ?? false;
    setInput(current =>
      append && current !== '' ? `${current} ${draftPrompt.text}` : draftPrompt.text,
    );
    onDraftConsumed?.();
    if (append && !wasInChat) return;
    requestAnimationFrame(() => {
      const el = composerRef.current;
      if (el === null) return;
      el.focus();
      el.setSelectionRange(el.value.length, el.value.length);
    });
  }, [draftPrompt?.nonce]);
  const [mcpInfo, setMcpInfo] = useState<{ url: string; token: string } | null>(null);
  // Outside access is opt-in: with it off there are no details to show, only how to turn it on.
  const [mcpAccessOff, setMcpAccessOff] = useState(false);
  const [mcpCopied, setMcpCopied] = useState(false);
  // Anchor for the "New Chat" dropdown (new chat + recent sessions), replacing the old
  // full-panel history view.
  const [chatMenuAnchor, setChatMenuAnchor] = useState<null | HTMLElement>(null);
  // Deletes a saved chat from its menu item and keeps the focus in the menu, on the next item
  // or, after the last one, on the item before.
  const deleteSessionByKey = (item: HTMLElement, id: string) => {
    const items = Array.from(
      item
        .closest('[role="menu"]')
        ?.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)') ?? [],
    );
    const at = items.indexOf(item);
    (items[at + 1] ?? items[at - 1])?.focus();
    onDeleteSession(id);
  };

  // Saved conversations for the history menu (the current not-yet-used session isn't listed).
  const savedSessions = useMemo(() => sessions.filter(s => s.title !== ''), [sessions]);

  // The active saved conversation, if any — its title labels the dropdown button; otherwise the
  // button reads "New chat" (#1619).
  const currentSession = useMemo(
    () => savedSessions.find(s => s.id === currentSessionId),
    [savedSessions, currentSessionId],
  );

  const currentProvider = useMemo(
    () => providers.find(p => p.id === provider),
    [providers, provider],
  );
  const available = currentProvider?.available ?? false;
  const cmds = cliSetup(provider, navigator.userAgent.includes('Windows'));
  const guideLink = (
    <button
      className={styles.guideLink}
      type="button"
      onClick={() => void misc.openExternal(cmds.guideUrl)}
    >
      {t('editor.ai.setup.install_guide', { provider: currentProvider?.label ?? provider })}
      <OpenInNewIcon fontSize="inherit" />
    </button>
  );

  // In-app sign-in without a CLI (#1531): install the official CLI on demand + drive its
  // subscription login (browser OAuth), streaming steps here. On success we re-detect so
  // the provider flips to available.
  const {
    signIn,
    start: handleSignIn,
    cancel: handleCancelSignIn,
    submitCode: handleSubmitSignInCode,
  } = useCliSignIn(provider, onRecheck);

  useEffect(() => {
    const el = transcriptRef.current;
    if (el !== null) el.scrollTop = el.scrollHeight;
  }, [messages, busy]);

  // No CLI installed? Reveal the scene's MCP server so a tool the user already has (Claude
  // Desktop, the VS Code extension, …) can connect to this scene instead (#1502), provided
  // outside access is on. Fetch it only once detection has concluded the CLI is missing —
  // asking for the info starts the server, which we shouldn't do when the CLI path works.
  useEffect(() => {
    if (detecting || available) {
      setMcpInfo(null);
      return;
    }
    let cancelled = false;
    void aiPreload.getMcpServerInfo().then(
      info => {
        if (cancelled) return;
        setMcpInfo(info);
        setMcpAccessOff(info === null);
      },
      () => !cancelled && setMcpInfo(null),
    );
    return () => {
      cancelled = true;
    };
  }, [detecting, available]);

  const mcpConfigSnippet = useMemo(
    () =>
      mcpInfo === null
        ? null
        : JSON.stringify(
            {
              mcpServers: {
                'creator-hub': {
                  type: 'http',
                  url: mcpInfo.url,
                  headers: { Authorization: `Bearer ${mcpInfo.token}` },
                },
              },
            },
            null,
            2,
          ),
    [mcpInfo],
  );

  const handleCopyMcpConfig = useCallback(() => {
    if (mcpConfigSnippet === null) return;
    void misc.copyToClipboard(mcpConfigSnippet).then(() => {
      setMcpCopied(true);
      setTimeout(() => setMcpCopied(false), 2000);
    });
  }, [mcpConfigSnippet]);

  // Resolve dropped/picked/pasted files to attachments and append them, keeping within the cap.
  const addFiles = useCallback(async (files: FileList | File[]) => {
    const list = Array.from(files);
    if (list.length === 0) return;
    const resolved = (await Promise.all(list.map(fileToAttachment))).filter(
      (a): a is AiAttachment => a !== null,
    );
    if (resolved.length === 0) return;
    setAttachments(prev => [...prev, ...resolved].slice(0, MAX_ATTACHMENTS));
  }, []);

  const removeAttachment = useCallback((index: number) => {
    setAttachments(prev => prev.filter((_, i) => i !== index));
  }, []);

  const handleSend = useCallback(() => {
    if (busy) return;
    const text = input.trim();
    if (text === '' && attachments.length === 0 && recording === null) return;
    onSend(text, attachments.length > 0 ? attachments : undefined);
    setInput('');
    setAttachments([]);
  }, [busy, input, attachments, recording, onSend]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend],
  );

  const handleDragOver = useCallback(
    (e: React.DragEvent) => {
      if (!isFileDrag(e)) return;
      // Always swallow the default: an unprevented file drop makes the window try to open the
      // file (will-navigate then blocks it, but the flash is ugly). Only signal/accept when the
      // provider is available.
      e.preventDefault();
      e.dataTransfer.dropEffect = available ? 'copy' : 'none';
      if (available) setDragging(true);
    },
    [available],
  );

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    // Ignore leaves into a child element — only clear when the pointer exits the composer.
    if (e.currentTarget.contains(e.relatedTarget as Node | null)) return;
    setDragging(false);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      if (!isFileDrag(e)) return;
      e.preventDefault();
      setDragging(false);
      if (available && e.dataTransfer.files.length > 0) void addFiles(e.dataTransfer.files);
    },
    [available, addFiles],
  );

  // Pasting an image from the clipboard attaches it (it has no disk path, so it rides as a data URL).
  const handlePaste = useCallback(
    (e: React.ClipboardEvent) => {
      if (!available || e.clipboardData.files.length === 0) return;
      e.preventDefault();
      void addFiles(e.clipboardData.files);
    },
    [available, addFiles],
  );

  const handleFilePick = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      if (e.target.files !== null) void addFiles(e.target.files);
      e.target.value = ''; // reset so the same file can be picked again
    },
    [addFiles],
  );

  const renderSetup = () => {
    return (
      <div className={styles.setupBox}>
        <strong>{t('editor.ai.setup.title')}</strong>
        <span>{t('editor.ai.setup.description')}</span>
        {currentProvider?.desktopInstalled === true && (
          <span>{t('editor.ai.setup.desktop_detected')}</span>
        )}
        {provider === 'gemini' && <span>{t('editor.ai.setup.gemini_plan_note')}</span>}
        {currentProvider?.managedSignIn === true && (
          <>
            <Button
              variant="text"
              size="sm"
              className={styles.textButton}
              disabled={signIn.busy}
              startIcon={
                signIn.busy ? (
                  <Spinner
                    ring="wide"
                    size={16}
                    role="progressbar"
                  />
                ) : undefined
              }
              onClick={handleSignIn}
            >
              {t('editor.ai.setup.signin_button', {
                provider: currentProvider?.label ?? provider,
              })}
            </Button>
            {signIn.busy && signIn.message !== '' && <span>{signIn.message}</span>}
            {signIn.busy && (
              <Button
                variant="text"
                tone="neutral"
                size="sm"
                className={styles.textButton}
                onClick={handleCancelSignIn}
              >
                {t('editor.ai.setup.signin_cancel')}
              </Button>
            )}
            {signIn.url !== null && (
              <div className={styles.setupStep}>
                <span>{t('editor.ai.setup.signin_browser')}</span>
                <pre className={styles.commandLine}>{signIn.url}</pre>
              </div>
            )}
            {signIn.busy && signIn.url !== null && signIn.acceptsCode && (
              <SignInCodeField onSubmit={handleSubmitSignInCode} />
            )}
            {signIn.error !== null && <div className={styles.errorRow}>{signIn.error}</div>}
            <hr className={styles.setupDivider} />
          </>
        )}
        {currentProvider?.managedSignIn === true && (
          <span>{t('editor.ai.setup.manual_title')}</span>
        )}
        <div className={styles.setupStep}>
          <span>{t('editor.ai.setup.install')}</span>
          <pre className={styles.commandLine}>{cmds.install}</pre>
          {guideLink}
        </div>
        <div className={styles.setupStep}>
          <span>{t('editor.ai.setup.signin')}</span>
          <pre className={styles.commandLine}>{cmds.signin}</pre>
        </div>
        <Button
          variant="text"
          tone="neutral"
          size="sm"
          className={styles.textButton}
          startIcon={
            detecting ? (
              <Spinner
                ring="wide"
                size={16}
                role="progressbar"
              />
            ) : (
              <RefreshIcon />
            )
          }
          disabled={detecting}
          onClick={onRecheck}
        >
          {t('editor.ai.setup.recheck')}
        </Button>
        <hr className={styles.setupDivider} />
        <div className={styles.setupAlt}>
          <strong>{t('editor.ai.setup.alt_title')}</strong>
          <span>{t('editor.ai.setup.alt_description')}</span>
          {mcpAccessOff ? (
            <span>{t('editor.ai.setup.alt_access_off')}</span>
          ) : mcpConfigSnippet === null ? (
            <Spinner
              ring="wide"
              size={16}
              role="progressbar"
            />
          ) : (
            <>
              <pre className={styles.commandLine}>{mcpConfigSnippet}</pre>
              <Button
                variant="text"
                tone="neutral"
                size="sm"
                className={styles.textButton}
                startIcon={<ContentCopyIcon fontSize="small" />}
                onClick={handleCopyMcpConfig}
              >
                {mcpCopied ? t('editor.ai.setup.alt_copied') : t('editor.ai.setup.alt_copy')}
              </Button>
              <span>{t('editor.ai.setup.alt_note')}</span>
            </>
          )}
        </div>
      </div>
    );
  };

  const renderTranscript = () => {
    // "Undo AI changes" is offered only on the latest turn: undo is a shared stack, so an
    // older turn's entries aren't on top and can't be cleanly reverted in isolation.
    const lastId = messages[messages.length - 1]?.id;
    const transcript = messages.map(msg =>
      msg.role === 'user' ? (
        <div
          className={styles.userBubble}
          key={msg.id}
        >
          {messageText(msg)}
          {msg.attachments !== undefined && msg.attachments.length > 0 && (
            <div
              className={styles.attachmentBar}
              data-bubble={messageText(msg) !== '' ? 'spaced' : 'flush'}
            >
              {msg.attachments.map((att, i) => (
                <div
                  className={styles.attachmentChip}
                  key={`${att.name}-${i}`}
                >
                  <AttachmentIcon kind={att.kind} />
                  <span
                    className={styles.ellipsis}
                    title={att.name}
                  >
                    {att.name}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      ) : (
        <div
          className={styles.assistantBubble}
          key={msg.id}
        >
          {msg.parts.map((part, i) => {
            switch (part.kind) {
              case 'text':
                return (
                  <div
                    className={styles.assistantText}
                    key={i}
                  >
                    <Markdown options={MARKDOWN_OPTIONS}>{part.text}</Markdown>
                  </div>
                );
              case 'tool':
                return (
                  <div
                    className={styles.toolChip}
                    key={i}
                  >
                    <span>{toolChipLabel(part.tool)}</span>
                    {part.detail !== '' && <span className={styles.ellipsis}>{part.detail}</span>}
                  </div>
                );
              case 'image':
                return (
                  <img
                    className={styles.assistantImage}
                    key={i}
                    src={part.dataUrl}
                    alt={t('editor.ai.screenshot_alt')}
                  />
                );
              case 'prompt':
                return (
                  <PromptBlock
                    key={i}
                    prompt={part.prompt}
                    onAnswer={answer => onAnswerPrompt(part.prompt.id, answer)}
                  />
                );
              default: {
                // Exhaustiveness: a new AiPart variant fails to compile until handled here.
                const _exhaustive: never = part;
                return _exhaustive;
              }
            }
          })}
          {msg.error !== undefined && (
            <div className={styles.errorRow}>
              {msg.error}
              <div>
                <Button
                  variant="text"
                  tone="neutral"
                  size="sm"
                  className={styles.textButton}
                  onClick={() => {
                    const lastUser = [...messages].reverse().find(m => m.role === 'user');
                    if (lastUser === undefined) return;
                    const prompt = messageText(lastUser);
                    setInput(current => (current.trim() === prompt.trim() ? '' : current));
                    onSend(prompt);
                  }}
                >
                  {t('editor.ai.retry')}
                </Button>
              </div>
            </div>
          )}
          {msg.id === lastId &&
            msg.done &&
            msg.error === undefined &&
            (msg.mutations ?? 0) > 0 &&
            !msg.reverted && (
              <Button
                variant="text"
                tone="neutral"
                size="sm"
                className={styles.textButton}
                startIcon={<UndoIcon fontSize="small" />}
                onClick={() => onRevertTurn(msg.id, msg.mutations ?? 0)}
              >
                {t('editor.ai.revert')}
              </Button>
            )}
        </div>
      ),
    );
    return (
      <>
        <div className={styles.introMessage}>{t('editor.ai.empty')}</div>
        {transcript}
        {busy && !isAwaitingAnswer(messages.at(-1)) && (
          <div
            className={styles.thinkingRow}
            role="status"
          >
            <Spinner
              ring="wide"
              size={12}
              role="progressbar"
            />
            {t('editor.ai.thinking')}
          </div>
        )}
      </>
    );
  };

  const panelStyle =
    width !== undefined ? ({ '--panel-width': `${width}px` } as CSSProperties) : undefined;

  return (
    <div
      ref={panelRef}
      className={styles.panel}
      data-fill={detached || undefined}
      style={panelStyle}
      aria-label="ai-chat-panel"
      data-recording-area="AI assistant panel"
    >
      <div className={styles.panelHeader}>
        <div className={styles.headerTitle}>
          {t('editor.ai.title')}
          {title !== undefined && title !== '' ? ` — ${title}` : ''}
        </div>
        <div className={styles.headerActions}>
          {!detached && onPopOut !== undefined && (
            <Tooltip
              label={t('editor.ai.pop_out')}
              side="bottom"
              portal
            >
              <IconButton
                size="small"
                aria-label={t('editor.ai.pop_out')}
                onClick={onPopOut}
              >
                <OpenInNewIcon fontSize="small" />
              </IconButton>
            </Tooltip>
          )}
          <Tooltip
            label={t('editor.ai.close')}
            side="bottom"
            portal
          >
            <IconButton
              size="small"
              aria-label={t('editor.ai.close')}
              onClick={onClose}
            >
              <CloseIcon fontSize="small" />
            </IconButton>
          </Tooltip>
        </div>
      </div>

      <>
        <div className={styles.toolbar}>
          <button
            className={styles.toolbarPill}
            type="button"
            data-open={chatMenuAnchor !== null || undefined}
            aria-label={t('editor.ai.new_chat')}
            aria-haspopup="menu"
            aria-expanded={chatMenuAnchor !== null}
            onClick={e => {
              const pill = e.currentTarget;
              setChatMenuAnchor(prev => (prev === null ? pill : null));
            }}
          >
            <AddCommentOutlinedIcon className={styles.menuIcon} />
            <span className={styles.ellipsis}>
              {currentSession !== undefined ? currentSession.title : t('editor.ai.new_chat')}
            </span>
            <KeyboardArrowDownIcon
              fontSize="small"
              className={styles.pillChevron}
            />
          </button>
          <Menu
            open={chatMenuAnchor !== null}
            anchor={chatMenuAnchor}
            onClose={() => setChatMenuAnchor(null)}
            placement="bottom-start"
            offset={10}
            className={cx(styles.menu, styles.chatMenu)}
          >
            <MenuItem
              className={styles.menuItem}
              disabled={busy || messages.length === 0}
              onClick={() => {
                onNewChat();
                setChatMenuAnchor(null);
              }}
            >
              <AddCommentOutlinedIcon className={styles.menuIcon} />
              {t('editor.ai.new_chat')}
            </MenuItem>
            {savedSessions.length > 0 && (
              <>
                <MenuSeparator />
                <div className={styles.menuSectionLabel}>{t('editor.ai.history.title')}</div>
              </>
            )}
            {savedSessions.map(s => (
              <div
                key={s.id}
                className={styles.sessionRow}
                data-current={s.id === currentSessionId || undefined}
              >
                <MenuItem
                  className={styles.sessionItem}
                  selected={s.id === currentSessionId}
                  aria-keyshortcuts="Delete"
                  onKeyDown={event => {
                    if (event.key === 'Delete' || event.key === 'Backspace') {
                      event.preventDefault();
                      deleteSessionByKey(event.currentTarget, s.id);
                    }
                  }}
                  onClick={() => {
                    onSwitchSession(s.id);
                    setChatMenuAnchor(null);
                  }}
                >
                  <CheckIcon
                    className={cx(styles.menuIcon, s.id !== currentSessionId && styles.hiddenMark)}
                  />
                  <span className={styles.sessionText}>
                    <span className={styles.ellipsis}>{s.title}</span>
                    <span className={styles.sessionWhen}>{formatWhen(s.updatedAt)}</span>
                  </span>
                </MenuItem>
                {/* The keyboard deletes from the item above with the Delete key. */}
                <IconButton
                  size="small"
                  tabIndex={-1}
                  aria-label={t('editor.ai.history.delete')}
                  onClick={() => onDeleteSession(s.id)}
                >
                  <DeleteOutlineIcon fontSize="small" />
                </IconButton>
              </div>
            ))}
          </Menu>
          {providers.length > 1 && (
            <Select
              value={provider}
              onChange={onProviderChange}
              disabled={busy}
              icon={<KeyboardArrowDownIcon />}
              placement="bottom-end"
              className={styles.providerSelect}
              menuClassName={cx(styles.menu, styles.providerMenu)}
              // Show just the active agent's name in the closed box (with a subtle "sign in"
              // cue when it isn't ready), not the full per-row status.
              renderValue={id => {
                const p = providers.find(x => x.id === id);
                if (p === undefined) return id;
                return (
                  <span className={styles.providerOption}>
                    <span>{p.label}</span>
                    {!p.available && (
                      <span className={styles.providerValueHint}>
                        {t('editor.ai.provider_signin')}
                      </span>
                    )}
                  </span>
                );
              }}
            >
              {/* Every agent is selectable — picking one that isn't signed in yet switches the
                  panel to its sign-in screen, so users can add and swap agents on the fly. */}
              {providers.map(p => (
                <SelectOption
                  key={p.id}
                  value={p.id}
                  label={p.label}
                  className={styles.providerRow}
                >
                  <CheckIcon
                    fontSize="small"
                    className={p.id === provider ? undefined : styles.hiddenMark}
                  />
                  <span className={styles.providerOption}>
                    <span>{p.label}</span>
                    {!p.available && (
                      <span className={styles.providerHint}>
                        {p.reason ?? t('editor.ai.provider_signin')}
                      </span>
                    )}
                  </span>
                </SelectOption>
              ))}
            </Select>
          )}
        </div>

        <div
          className={styles.transcript}
          ref={transcriptRef}
        >
          {available ? renderTranscript() : renderSetup()}
        </div>

        {available &&
          currentProvider?.id === 'claude' &&
          isCliVersionOutdated(currentProvider.version, MIN_CLAUDE_CLI_VERSION) && (
            <div className={styles.outdatedHint}>
              <WarningCircleIcon size={28} />
              <span>{t('editor.ai.outdated', { version: currentProvider.version ?? '' })}</span>
            </div>
          )}

        {available && currentProvider?.needsLogin === true && (
          <div className={styles.outdatedHint}>
            <WarningCircleIcon size={28} />
            <div className={styles.signInHintBody}>
              <strong>
                {t('editor.ai.setup.needs_login_title', { provider: currentProvider.label })}
              </strong>
              {currentProvider.managedSignIn ? (
                <span>{t('editor.ai.setup.needs_login_managed_help')}</span>
              ) : (
                <>
                  <span>{t('editor.ai.setup.needs_login_help')}</span>
                  {cmds.relogin.map(step => (
                    <pre
                      className={styles.commandLine}
                      key={step}
                    >
                      {step}
                    </pre>
                  ))}
                </>
              )}
              <span>
                {t('editor.ai.setup.api_key_alternative', {
                  envVar: cmds.apiKeyEnv,
                  setting: t('modal.app_settings.fields.ai_api_key.label'),
                })}
              </span>
              {guideLink}
            </div>
          </div>
        )}

        {available && !billingDismissed && (
          <div className={styles.billingCard}>
            <span className={styles.billingTitle}>{t('editor.ai.billing_title')}</span>
            <span className={styles.billingBody}>{t('editor.ai.billing')}</span>
            <Button
              variant="text"
              tone="neutral"
              size="sm"
              className={styles.billingButton}
              onClick={onDismissBilling}
            >
              {t('editor.ai.billing_dismiss')}
            </Button>
          </div>
        )}

        {available && selection.length > 0 && (
          <div className={styles.selectionBar}>
            <HighlightAltIcon fontSize="small" />
            <span className={styles.selectionNames}>
              {t('editor.ai.selection', {
                names: selection.map(s => (s.name !== '' ? s.name : `#${s.id}`)).join(', '),
              })}
            </span>
            <ClearMentionButton
              label={t('editor.ai.selection_clear')}
              onClick={onClearSelection}
            />
          </div>
        )}

        {available &&
          assetMentions.map(mention => (
            <div
              className={styles.selectionBar}
              key={assetMentionKey(mention)}
            >
              <InsertDriveFileOutlinedIcon fontSize="small" />
              {'path' in mention ? (
                <span
                  className={styles.selectionNames}
                  title={mention.path}
                >
                  {t('editor.ai.asset_mention', {
                    name: mention.path.split('/').pop() ?? mention.path,
                  })}
                </span>
              ) : (
                <span className={styles.selectionNames}>
                  {t('editor.ai.asset_mention_built_in', { name: mention.builtInEmote })}
                </span>
              )}
              <ClearMentionButton
                label={t('editor.ai.asset_mention_remove')}
                onClick={() => onRemoveAssetMention(mention)}
              />
            </div>
          ))}

        {recorder?.status === 'recording' && onToggleRecording !== undefined && (
          <div className={recordRowStyles.recordRow}>
            <RecordingCard
              startedAt={recorder.startedAt}
              micLabel={recorder.micLabel}
              actionCount={recorder.actionCount}
              onStop={onToggleRecording}
              onDiscard={onCancelRecording}
              focusStopOnMount={recordingJustStarted}
              onFocusRelease={() => composerRef.current?.focus()}
            />
          </div>
        )}
        {recorder?.status === 'stopping' && (
          <div className={recordRowStyles.recordRow}>
            <TranscribingCard
              startedAt={recorder.startedAt}
              stoppedAt={recorder.stoppedAt}
              actionCount={recorder.actionCount}
              tools={recorder.tools}
              onSkip={onSkipTranscription}
              onFocusRelease={() => composerRef.current?.focus()}
            />
          </div>
        )}

        <div
          className={styles.composer}
          data-dragging={dragging || undefined}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          {dragging && (
            <div className={styles.dropHint}>
              <AttachFileIcon fontSize="small" />
              {t('editor.ai.attach.drop_hint')}
            </div>
          )}
          {attachments.length > 0 && (
            <div className={styles.attachmentBar}>
              {attachments.map((att, i) => (
                <div
                  className={styles.attachmentChip}
                  key={`${att.name}-${i}`}
                >
                  <AttachmentIcon kind={att.kind} />
                  <span
                    className={styles.ellipsis}
                    title={att.name}
                  >
                    {att.name}
                  </span>
                  <Tooltip
                    label={t('editor.ai.attach.remove')}
                    side="bottom"
                    portal
                    keepInViewport
                  >
                    <IconButton
                      className={styles.attachmentRemove}
                      aria-label={t('editor.ai.attach.remove')}
                      onClick={() => removeAttachment(i)}
                    >
                      <CloseIcon />
                    </IconButton>
                  </Tooltip>
                </div>
              ))}
            </div>
          )}
          {available && recording !== null && (
            <div className={styles.attachmentBar}>
              <div
                className={cx(styles.attachmentChip, styles.recordingChip)}
                data-testid="ai-recording-chip"
              >
                <MicrophoneIcon size={16} />
                <span
                  className={styles.ellipsis}
                  title={recording.sessions.map(one => one.dir).join('\n')}
                >
                  {recordingChipLabel(recording)}
                </span>
                <Tooltip
                  label={t('editor.ai.recording.remove')}
                  side="bottom"
                  portal
                  keepInViewport
                >
                  <IconButton
                    className={styles.attachmentRemove}
                    aria-label={t('editor.ai.recording.remove')}
                    onClick={onRemoveRecording}
                  >
                    <CloseIcon />
                  </IconButton>
                </Tooltip>
              </div>
            </div>
          )}
          <input
            ref={fileInputRef}
            type="file"
            multiple
            hidden
            onChange={handleFilePick}
          />
          <TextField
            fullWidth
            multiline
            maxRows={6}
            size="small"
            tone="neutral"
            autoFocus
            inputRef={composerRef}
            className={styles.composerField}
            placeholder={t('editor.ai.placeholder')}
            value={input}
            disabled={!available}
            onChange={e => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
            startAdornment={
              <>
                <Tooltip
                  label={t('editor.ai.attach.button')}
                  side="bottom"
                  portal
                  keepInViewport
                >
                  <IconButton
                    className={styles.attachButton}
                    aria-label={t('editor.ai.attach.button')}
                    disabled={!available || attachments.length >= MAX_ATTACHMENTS}
                    onClick={() => fileInputRef.current?.click()}
                  >
                    <AttachFileIcon fontSize="small" />
                  </IconButton>
                </Tooltip>
                {recorder !== undefined &&
                  onToggleRecording !== undefined &&
                  (recorder.status === 'idle' || recorder.status === 'starting') && (
                    <span className={styles.composerRecord}>
                      <RecordButton
                        status={recorder.status}
                        startedAt={recorder.startedAt}
                        micLabel={recorder.micLabel}
                        tools={recorder.tools}
                        disabled={!available}
                        onClick={onToggleRecording}
                      />
                    </span>
                  )}
              </>
            }
            endAdornment={
              busy ? (
                <Tooltip
                  label={t('editor.ai.stop')}
                  side="bottom"
                  portal
                  keepInViewport
                >
                  <IconButton
                    className={styles.sendButton}
                    aria-label={t('editor.ai.stop')}
                    onClick={onStop}
                  >
                    <StopIcon fontSize="small" />
                  </IconButton>
                </Tooltip>
              ) : (
                <Tooltip
                  label={t('editor.ai.send')}
                  side="bottom"
                  portal
                  keepInViewport
                >
                  <IconButton
                    className={styles.sendButton}
                    aria-label={t('editor.ai.send')}
                    disabled={
                      !available ||
                      (input.trim() === '' && attachments.length === 0 && recording === null)
                    }
                    onClick={handleSend}
                  >
                    <ArrowUpwardIcon fontSize="small" />
                  </IconButton>
                </Tooltip>
              )
            }
          />
        </div>
      </>
    </div>
  );
}
