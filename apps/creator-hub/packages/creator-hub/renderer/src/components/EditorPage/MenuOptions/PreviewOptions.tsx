import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { cx } from '/@/lib/cx';
import { ChevronRightIcon } from '/@/components/Icons/material';
import Checkbox from '@ui/atoms/Checkbox';
import Radio from '@ui/atoms/Radio';
import Text from '@ui/atoms/Text';
import Tooltip from '@ui/atoms/Tooltip';

import { scene } from '#preload';

import { PREVIEW_CLIENT } from '/shared/types/settings';
import { t } from '/@/modules/store/translation/utils';

import type { PreviewOptionsProps } from './types';

const DESKTOP_SUBMENU_CLOSE_DELAY_MS = 300;

export function PreviewOptions({
  onChange,
  options,
  onShowMobileQR,
  supportsMultiInstance,
  supportsMcp,
  projectPath,
}: PreviewOptionsProps) {
  const [terrainHiddenByScene, setTerrainHiddenByScene] = useState(false);
  const [desktopOptionsOpen, setDesktopOptionsOpen] = useState(false);
  // The flyout normally opens to the right of the Desktop Client row (per design), but that
  // row sits in a popover already right-aligned to the toolbar's Preview button — near the
  // window's right edge in practice — so a fixed-right flyout gets clipped off-screen. Flip
  // it to the left whenever there isn't enough room on the right.
  const desktopRowRef = useRef<HTMLDivElement>(null);
  // null until measured for the current open — rendering nothing meanwhile (rather than a
  // default side) avoids a visible flash-then-flip when the flyout needs to go left.
  const [submenuAlign, setSubmenuAlign] = useState<'left' | 'right' | null>(null);

  const isDesktopClient = options.client === PREVIEW_CLIENT.DESKTOP;

  useEffect(() => {
    let cancelled = false;
    scene
      .getScene(projectPath)
      .then(sceneJson => {
        if (!cancelled) {
          setTerrainHiddenByScene(
            (sceneJson as { landscapeTerrain?: boolean }).landscapeTerrain === false,
          );
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [projectPath]);

  const handleChange = useCallback(
    (newOptions: Partial<PreviewOptionsProps['options']>) => () => {
      onChange({ ...options, ...newOptions });
    },
    [onChange, options],
  );

  const handleClientSelect = useCallback(
    (client: PREVIEW_CLIENT) => () => {
      onChange({ ...options, client });
    },
    [onChange, options],
  );

  // Closing on the instant of mouseleave drops the flyout while the pointer is still crossing
  // the 8px gap to it, or cutting diagonally over the neighbouring rows to reach it. The grace
  // period is cancelled by re-entering the row, which includes entering the flyout (a child).
  const closeDesktopOptionsTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const cancelCloseDesktopOptions = useCallback(() => {
    clearTimeout(closeDesktopOptionsTimer.current);
    closeDesktopOptionsTimer.current = undefined;
  }, []);

  useEffect(() => cancelCloseDesktopOptions, [cancelCloseDesktopOptions]);

  const handleDesktopRowMouseEnter = useCallback(() => {
    cancelCloseDesktopOptions();
    setDesktopOptionsOpen(true);
  }, [cancelCloseDesktopOptions]);

  const handleDesktopRowMouseLeave = useCallback(() => {
    cancelCloseDesktopOptions();
    closeDesktopOptionsTimer.current = setTimeout(
      () => setDesktopOptionsOpen(false),
      DESKTOP_SUBMENU_CLOSE_DELAY_MS,
    );
  }, [cancelCloseDesktopOptions]);

  const showDesktopSubmenu = isDesktopClient && desktopOptionsOpen;

  useLayoutEffect(() => {
    if (!showDesktopSubmenu || !desktopRowRef.current) {
      // Reset so the next hover re-measures instead of briefly showing last time's side.
      setSubmenuAlign(null);
      return;
    }
    // Measured a frame late (rAF) on purpose: the outer Popper (popper.js) applies its own
    // position transform in its own effect, and ordering against that isn't guaranteed —
    // measuring in the same layout-effect pass can read the row's pre-positioned rect. The
    // flyout stays unrendered (submenuAlign is null) until this resolves, so there's nothing
    // to flash on the wrong side in the meantime.
    const raf = requestAnimationFrame(() => {
      if (!desktopRowRef.current) return;
      const SUBMENU_WIDTH_WITH_MARGIN = 308; // width: 300px + margin-left: 8px
      const { right } = desktopRowRef.current.getBoundingClientRect();
      const hasRoomOnRight = window.innerWidth - right >= SUBMENU_WIDTH_WITH_MARGIN;
      setSubmenuAlign(hasRoomOnRight ? 'right' : 'left');
    });
    return () => cancelAnimationFrame(raf);
  }, [showDesktopSubmenu]);

  return (
    <div className="PreviewOptions">
      <div className="options">
        <span className="title">{t('editor.header.actions.preview_options.title')}</span>
        <div
          ref={desktopRowRef}
          className="client-row"
          onMouseEnter={handleDesktopRowMouseEnter}
          onMouseLeave={handleDesktopRowMouseLeave}
        >
          <Radio
            className="control"
            value={PREVIEW_CLIENT.DESKTOP}
            checked={isDesktopClient}
            onChange={handleClientSelect(PREVIEW_CLIENT.DESKTOP)}
          >
            {t('editor.header.actions.preview_options.client.desktop')}
          </Radio>
          <ChevronRightIcon
            className="arrow"
            aria-hidden="true"
          />
          {showDesktopSubmenu && submenuAlign && (
            <div className={cx('client-submenu', { 'align-left': submenuAlign === 'left' })}>
              <div className="options">
                <Tooltip
                  className="tooltip-row"
                  label={t(
                    'editor.header.actions.preview_options.landscape_terrain_disabled_by_scene',
                  )}
                  side="left"
                  portal
                  disabled={!terrainHiddenByScene}
                >
                  <div className="checkbox-row">
                    <Checkbox
                      className="control"
                      checked={terrainHiddenByScene ? false : !!options.enableLandscapeTerrains}
                      disabled={terrainHiddenByScene}
                      onChange={handleChange({
                        enableLandscapeTerrains: !options.enableLandscapeTerrains,
                      })}
                    >
                      {t('editor.header.actions.preview_options.landscape_terrain_enabled')}
                    </Checkbox>
                  </div>
                </Tooltip>
                {supportsMultiInstance && (
                  <div className="checkbox-row">
                    <Checkbox
                      className="control"
                      checked={!!options.multiInstance}
                      onChange={handleChange({ multiInstance: !options.multiInstance })}
                    >
                      {t('editor.header.actions.preview_options.multi_instance')}
                    </Checkbox>
                  </div>
                )}
                {supportsMcp && (
                  <div className="checkbox-row">
                    <Checkbox
                      className="control"
                      checked={!!options.mcp}
                      onChange={handleChange({ mcp: !options.mcp })}
                    >
                      {t('editor.header.actions.preview_options.mcp')}
                    </Checkbox>
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
        <div className="client-row">
          <Radio
            className="control"
            value={PREVIEW_CLIENT.BEVY_WEB}
            checked={!isDesktopClient}
            onChange={handleClientSelect(PREVIEW_CLIENT.BEVY_WEB)}
          >
            {t('editor.header.actions.preview_options.client.bevy_web')}
          </Radio>
        </div>
      </div>
      <hr className="divider" />
      <div className="options">
        <button
          type="button"
          className="option"
          onClick={onShowMobileQR}
        >
          <Text
            as="span"
            variant="body1"
          >
            {t('editor.header.actions.preview_options.mobile_preview')}
          </Text>
        </button>
      </div>
    </div>
  );
}
