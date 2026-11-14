import React, { useRef, useState } from 'react';

import styles from './RenameField.module.css';

interface RenameFieldProps {
  name: string;
  onSubmit: (next: string) => void;
  onCancel: () => void;
}

/** The 2D hierarchy's in-row rename field (the rail SearchBar's Editing look): Enter or blur commits, Escape cancels back to the row. */
export const RenameField: React.FC<RenameFieldProps> = ({ name, onSubmit, onCancel }) => {
  const [draft, setDraft] = useState(name);
  const done = useRef(false);

  const finish = (commit: boolean) => {
    if (done.current) return;
    done.current = true;
    const next = draft.trim();
    if (commit && next && next !== name) onSubmit(next);
    else onCancel();
  };

  return (
    <input
      className={styles.field}
      value={draft}
      autoFocus
      spellCheck={false}
      autoCorrect="off"
      autoCapitalize="off"
      aria-label={`Rename ${name}`}
      onClick={e => e.stopPropagation()}
      onDoubleClick={e => e.stopPropagation()}
      onChange={e => setDraft(e.target.value)}
      onKeyDown={e => {
        e.stopPropagation();
        if (e.key === 'Enter') finish(true);
        else if (e.key === 'Escape') {
          const row = e.currentTarget.closest<HTMLElement>('[role="treeitem"], [role="button"]');
          done.current = true;
          row?.focus();
          onCancel();
        }
      }}
      onBlur={() => finish(true)}
    />
  );
};

export default RenameField;
