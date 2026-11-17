import { useCallback, useEffect, useRef, useState } from 'react';
import { ClearAllIcon, OpenInNewIcon } from '/@/components/Icons/material';
import { AnsiLine } from '@dcl/inspector/dist/ansi-entrypoint';
import IconButton from '@ui/atoms/IconButton';
import Tooltip from '@ui/atoms/Tooltip';

import { consoleWindow as consolePreload, editor } from '#preload';
import { useDispatch } from '#store';
import { actions as translationActions } from '/@/modules/store/translation';
import { locales, t } from '/@/modules/store/translation/utils';
import type { Locale } from '/shared/types/translation';

import styles from './component.module.css';

type LogEntry = { id: number; text: string };

const LOG_BATCH_INTERVAL = 100;
const MAX_ENTRIES = 1000;
const SCROLL_THRESHOLD = 10;

// The detached console window (#1272). It renders the same preview debug output as the inline
// console tab, but in its own OS window. It keeps no store of its own: it subscribes to the
// main-process preview debugger directly (editor.attachSceneDebugger) and only relies on the
// main window to relay whether the preview is running (so it re-attaches on the next run).
export function ConsoleWindow() {
  const dispatch = useDispatch();
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [previewRunning, setPreviewRunning] = useState(true);
  const pendingRef = useRef<LogEntry[]>([]);
  const idRef = useRef(0);
  const logsRef = useRef<HTMLDivElement>(null);
  const isAtBottomRef = useRef(true);
  const prevRunningRef = useRef(true);

  const path = new URLSearchParams(window.location.search).get('path') ?? undefined;

  // Match the app's locale (passed on the window URL) so the chrome isn't stuck on English.
  useEffect(() => {
    const raw = new URLSearchParams(window.location.search).get('locale');
    if (raw !== null && (locales as string[]).includes(raw)) {
      dispatch(translationActions.changeLocale(raw as Locale));
    }
  }, [dispatch]);

  const clearEntries = useCallback(() => {
    pendingRef.current = [];
    setEntries([]);
  }, []);

  // Flush batched incoming lines into state, capped, on a timer (logs can be high-frequency).
  useEffect(() => {
    const timer = setInterval(() => {
      if (pendingRef.current.length === 0) return;
      const batch = pendingRef.current;
      pendingRef.current = [];
      setEntries(prev => {
        const next = prev.concat(batch);
        return next.length > MAX_ENTRIES ? next.slice(next.length - MAX_ENTRIES) : next;
      });
    }, LOG_BATCH_INTERVAL);
    return () => clearInterval(timer);
  }, []);

  const pushLines = useCallback((data: string | string[]) => {
    const lines = Array.isArray(data) ? data : data.split('\n');
    for (const line of lines) {
      if (line.trim() !== '') {
        pendingRef.current.push({ id: idRef.current++, text: line });
      }
    }
  }, []);

  // Learn whether the preview is running from the main window's relay.
  useEffect(() => {
    const { cleanup } = consolePreload.onConsoleMirrorState(state =>
      setPreviewRunning(state.previewRunning),
    );
    return cleanup;
  }, []);

  // Subscribe to the preview debugger while it's running; re-subscribe (fresh) on each new run.
  useEffect(() => {
    if (path === undefined) return;
    if (!previewRunning) {
      prevRunningRef.current = false;
      return;
    }
    // A new run (after a stop) starts with a clean console; the initial mount keeps its backlog.
    if (prevRunningRef.current === false) clearEntries();
    prevRunningRef.current = true;

    let aborted = false;
    let cleanupFn: (() => void) | undefined;
    editor
      .attachSceneDebugger(path, pushLines)
      .then(({ cleanup }) => {
        if (aborted) cleanup();
        else cleanupFn = cleanup;
      })
      // Preview may have exited before we could attach.
      .catch(() => {});
    return () => {
      aborted = true;
      cleanupFn?.();
    };
  }, [path, previewRunning, pushLines, clearEntries]);

  const handleScroll = useCallback(() => {
    const el = logsRef.current;
    if (!el) return;
    isAtBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight <= SCROLL_THRESHOLD;
  }, []);

  useEffect(() => {
    if (isAtBottomRef.current && logsRef.current) {
      logsRef.current.scrollTop = logsRef.current.scrollHeight;
    }
  }, [entries]);

  const onDock = useCallback(() => {
    void consolePreload.closeConsoleWindow();
  }, []);

  return (
    <div className={styles.root}>
      <div className={styles.header}>
        <Tooltip
          label={t('editor.console.clear')}
          side="bottom"
          portal
        >
          <IconButton
            className={styles.headerButton}
            size="small"
            onClick={clearEntries}
            aria-label={t('editor.console.clear')}
          >
            <ClearAllIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <Tooltip
          label={t('editor.console.dock')}
          side="bottom"
          portal
        >
          <IconButton
            className={styles.headerButton}
            size="small"
            onClick={onDock}
            aria-label={t('editor.console.dock')}
          >
            <OpenInNewIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      </div>
      {entries.length > 0 ? (
        <div
          className={styles.logs}
          ref={logsRef}
          onScroll={handleScroll}
        >
          {entries.map(entry => (
            <AnsiLine
              key={entry.id}
              text={entry.text}
            />
          ))}
        </div>
      ) : (
        <div className={styles.placeholder}>{t('editor.console.empty')}</div>
      )}
    </div>
  );
}
