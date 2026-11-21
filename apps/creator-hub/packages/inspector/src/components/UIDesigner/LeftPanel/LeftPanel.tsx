import React, { useCallback, useRef, useState } from 'react';

import { analytics, Event } from '../../../lib/logic/analytics';
import { useAppSelector } from '../../../redux/hooks';
import { getMobileHudSelected, getSelectedNode } from '../../../redux/ui-designer';
import { Box } from '../../Box';
import { Close, Plus, Search } from '../../Icons/figma';
import { TextField } from '../../ui';
import { createRoot, useCodeState } from '../code/store';
import { useUINodeTree } from '../shared/useUINodeTree';
import { NodeTree } from './NodeTree';
import { WidgetPicker } from './WidgetPicker';
import { CodeRootsList } from './CodeRootsList';

import '../UIDesigner.css';

type SectionHeaderProps = {
  label: string;
  name: 'New GUI' | 'Add Node';
  /** True while an empty state points the author at this header's + chip. */
  pulsing: boolean;
  onClick: () => void;
} & Pick<React.ButtonHTMLAttributes<HTMLButtonElement>, 'aria-haspopup' | 'aria-expanded'>;

const SectionHeader = React.forwardRef<HTMLSpanElement, SectionHeaderProps>(
  ({ label, name, pulsing, onClick, ...popup }, chipRef) => (
    <button
      type="button"
      className="ui-designer-rail-header"
      aria-label={name}
      data-pulsing={pulsing ? 'true' : undefined}
      onClick={onClick}
      {...popup}
    >
      <span>{label}</span>
      <span
        ref={chipRef}
        className="ui-designer-rail-add"
        aria-hidden="true"
      >
        <Plus fontSize="inherit" />
      </span>
    </button>
  ),
);

const LeftPanel: React.FC = () => {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [search, setSearch] = useState('');
  const addNodeRef = useRef<HTMLSpanElement>(null);

  const handleSearchKeyDown = useCallback((event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape') {
      event.currentTarget.blur();
      setSearch('');
    }
  }, []);

  const selected = useAppSelector(getSelectedNode);
  const mobileHudSelected = useAppSelector(getMobileHudSelected);
  const { roots, filename, emptyRoot } = useCodeState();
  const tree = useUINodeTree();

  const showNodes = !mobileHudSelected && !!filename;
  const parent = selected ?? tree?.entity ?? null;

  return (
    <Box className="ui-designer-left-rail">
      <div
        className="ui-designer-rail-search"
        onContextMenu={e => e.stopPropagation()}
      >
        <TextField
          role="searchbox"
          autoComplete="off"
          placeholder="Search"
          aria-label="Search GUIs and nodes"
          value={search}
          onChange={e => setSearch(e.target.value)}
          onKeyDown={handleSearchKeyDown}
          leftIcon={<Search fontSize="inherit" />}
          rightIcon={
            search ? (
              <button
                type="button"
                className="ClearSearch"
                aria-label="Clear search"
                onClick={() => setSearch('')}
              >
                <Close fontSize="inherit" />
              </button>
            ) : undefined
          }
        />
      </div>
      <div className="ui-designer-rail-section ui-designer-rail-section-guis">
        <SectionHeader
          label="GUIs"
          name="New GUI"
          pulsing={roots.length === 0}
          onClick={() => {
            void createRoot();
            analytics.track(Event.CREATE_UI, {});
          }}
        />
        <CodeRootsList filter={search} />
      </div>
      {showNodes ? (
        <div className="ui-designer-rail-section ui-designer-rail-section-nodes">
          <SectionHeader
            ref={addNodeRef}
            label="Nodes"
            name="Add Node"
            pulsing={emptyRoot}
            aria-haspopup="dialog"
            aria-expanded={pickerOpen}
            onClick={() => setPickerOpen(true)}
          />
          <NodeTree filter={search} />
          {pickerOpen ? (
            <WidgetPicker
              anchorRef={addNodeRef}
              offset={8}
              onDismiss={() => setPickerOpen(false)}
              parent={emptyRoot ? null : parent}
            />
          ) : null}
        </div>
      ) : null}
    </Box>
  );
};

export default React.memo(LeftPanel);
export { LeftPanel };
