import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useDrag } from 'react-dnd';
import cx from 'classnames';

import { useAppDispatch, useAppSelector } from '../../../redux/hooks';
import { isMenuOpen } from '../../../hooks/useHotkey';
import { withContextMenu } from '../../../hoc/withContextMenu';
import { ContextMenu } from '../../Tree/ContextMenu';
import { getMobileHudSelected, selectMobileHud, selectNode } from '../../../redux/ui-designer';
import { UI_DESIGNER_DND_TYPE, type UIDesignerDragItem } from '../shared/dnd';
import { GUIRoot, Mobile } from '../../Icons/figma';
import { EyeIcon, EyeOffIcon } from '../icons';
import {
  type CodeRoot,
  duplicateRoot,
  removeRoot,
  renameRoot,
  selectRootFile,
  toggleTopLevel,
  useCodeState,
} from '../code/store';
import { loadMobileHudConfig, removeMobileHud } from '../MobileHud/mobile-hud-store';
import { RenameField } from './RenameField';

import './CodeRootsList.css';

const MobileHudRow: React.FC = () => {
  const dispatch = useAppDispatch();
  const active = useAppSelector(getMobileHudSelected);

  const onSelect = useCallback(() => {
    dispatch(selectMobileHud());
    void loadMobileHudConfig();
  }, [dispatch]);

  return (
    <div
      className={cx('ui-designer-mobile-hud-row', { 'is-active': active })}
      role="button"
      tabIndex={0}
      aria-current={active}
      onClick={onSelect}
      onKeyDown={e => {
        if (e.key !== 'Enter' && e.key !== ' ') return;
        e.preventDefault();
        onSelect();
      }}
      title="Customize the mobile touch-screen controls"
    >
      <Mobile fontSize="small" />
      <span className="ui-designer-mobile-hud-row-name">MobileHUD</span>
    </div>
  );
};

const noop = () => undefined;

type RootRowProps = {
  root: CodeRoot;
  active: boolean;
  editing: boolean;
  onSelect: () => void;
  onBeginEdit: () => void;
  onRename: (next: string) => void;
  onCancel: () => void;
  onRemove: () => void;
  onDuplicate: () => void;
};

const RootRowImpl: React.FC<RootRowProps & { contextMenuId: string }> = ({
  contextMenuId,
  root,
  active,
  editing,
  onSelect,
  onBeginEdit,
  onRename,
  onCancel,
  onRemove,
  onDuplicate,
}) => {
  const [{ isDragging }, drag] = useDrag<UIDesignerDragItem, unknown, { isDragging: boolean }>(
    () => ({
      type: UI_DESIGNER_DND_TYPE,
      item: { source: 'component', name: root.name },
      collect: monitor => ({ isDragging: monitor.isDragging() }),
    }),
    [root.name],
  );

  return (
    <div
      ref={editing ? undefined : (drag as unknown as React.Ref<HTMLDivElement>)}
      className={cx('ui-designer-code-root-row', {
        'is-active': active,
        'is-hidden': !root.topLevel,
      })}
      style={{ opacity: isDragging ? 0.4 : 1 }}
      role="button"
      tabIndex={0}
      aria-current={active}
      onClick={onSelect}
      onKeyDown={e => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onSelect();
          return;
        }
        if (isMenuOpen()) return;
        const remove = e.key === 'Delete' || e.key === 'Backspace';
        const duplicate = (e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'd';
        if (!remove && !duplicate) return;
        e.preventDefault();
        e.stopPropagation();
        if (remove) onRemove();
        else onDuplicate();
      }}
      title={`Drag onto the canvas to nest ${root.name} as a component`}
    >
      <ContextMenu
        id={contextMenuId}
        label="GUI Options"
        size="compact"
        enableAdd={false}
        onAddChild={noop}
        onEdit={onBeginEdit}
        onRemove={onRemove}
        onDuplicate={onDuplicate}
      />
      <GUIRoot fontSize="small" />
      {editing ? (
        <RenameField
          name={root.name}
          onSubmit={onRename}
          onCancel={onCancel}
        />
      ) : (
        <span
          className="ui-designer-code-root-name"
          onDoubleClick={e => {
            e.stopPropagation();
            onBeginEdit();
          }}
        >
          {root.name}
        </span>
      )}
      <div className="ui-designer-code-root-actions">
        <button
          type="button"
          className="ui-designer-code-root-toplevel ui-designer-toggle ui-designer-compact ui-designer-layer-toggle"
          title={
            root.topLevel
              ? 'Top-level: rendered on its own. Click to make it a nested-only component.'
              : 'Component: only rendered where it is nested. Click to make it top-level.'
          }
          aria-label={`Toggle top-level for ${root.name}`}
          aria-pressed={root.topLevel}
          onClick={e => {
            e.stopPropagation();
            void toggleTopLevel(root.filename);
          }}
        >
          {root.topLevel ? <EyeIcon /> : <EyeOffIcon />}
        </button>
      </div>
    </div>
  );
};

const RootRow = withContextMenu<RootRowProps>(RootRowImpl);

/** Code-mode roots list: files under src/ui/ backed by the code store; selecting a root loads its file as the active source the canvas edits. */
export const CodeRootsList: React.FC<{ filter?: string }> = ({ filter = '' }) => {
  const { roots, filename, parsed, error } = useCodeState();
  const dispatch = useAppDispatch();
  const mobileHudSelected = useAppSelector(getMobileHudSelected);

  const [editing, setEditing] = useState<string | null>(null);

  const beginEdit = useCallback((root: CodeRoot) => setEditing(root.filename), []);

  const commitRename = useCallback((root: CodeRoot, next: string) => {
    setEditing(null);
    void renameRoot(root.filename, next);
  }, []);

  const prevFile = useRef<string | null>(null);
  useEffect(() => {
    if (filename && filename !== prevFile.current && parsed?.root) {
      prevFile.current = filename;
      dispatch(selectNode({ node: parsed.root.entity }));
    }
  }, [filename, parsed, dispatch]);

  const hadRoots = useRef(false);
  useEffect(() => {
    const hasRoots = roots.length > 0;
    if (hadRoots.current && !hasRoots) {
      if (mobileHudSelected) dispatch(selectNode({ node: null }));
      void removeMobileHud();
    }
    hadRoots.current = hasRoots;
  }, [roots.length, mobileHudSelected, dispatch]);

  const handleSelect = useCallback(
    (root: CodeRoot) => {
      if (root.filename !== filename) void selectRootFile(root.filename);
      else dispatch(selectNode({ node: parsed?.root ? parsed.root.entity : null }));
    },
    [filename, parsed, dispatch],
  );

  const handleRemove = useCallback((root: CodeRoot) => {
    void removeRoot(root.filename);
  }, []);

  const term = filter.trim().toLowerCase();
  const shown = term ? roots.filter(root => root.name.toLowerCase().includes(term)) : roots;

  return (
    <div className="ui-designer-roots-list">
      {error ? (
        <div
          className="ui-designer-roots-error"
          role="alert"
        >
          {error}
        </div>
      ) : null}
      <div className="ui-designer-roots-tree">
        {roots.length > 0 && 'mobilehud'.includes(term) ? <MobileHudRow /> : null}
        {shown.map(root => (
          <RootRow
            key={root.filename}
            root={root}
            active={!mobileHudSelected && root.filename === filename}
            editing={editing === root.filename}
            onSelect={() => handleSelect(root)}
            onBeginEdit={() => beginEdit(root)}
            onRename={next => commitRename(root, next)}
            onCancel={() => setEditing(null)}
            onRemove={() => handleRemove(root)}
            onDuplicate={() => void duplicateRoot(root.filename)}
          />
        ))}
      </div>
    </div>
  );
};

export default React.memo(CodeRootsList);
