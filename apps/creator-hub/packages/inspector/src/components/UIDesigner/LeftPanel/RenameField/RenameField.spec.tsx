import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import { RenameField } from './RenameField';

const renderInRow = () => {
  const onSubmit = vi.fn();
  const onCancel = vi.fn();
  render(
    <div
      role="treeitem"
      tabIndex={0}
      aria-selected="false"
    >
      <RenameField
        name="Hud"
        onSubmit={onSubmit}
        onCancel={onCancel}
      />
    </div>,
  );
  const field = screen.getByLabelText('Rename Hud') as HTMLInputElement;
  return { field, onSubmit, onCancel, row: field.closest('[role="treeitem"]') as HTMLElement };
};

describe('RenameField', () => {
  afterEach(cleanup);

  it('should be named after the row and hold its name, focused', () => {
    const { field } = renderInRow();

    expect(field.value).toBe('Hud');
    expect(document.activeElement).toBe(field);
  });

  it('should commit a new name on Enter', () => {
    const { field, onSubmit } = renderInRow();
    fireEvent.change(field, { target: { value: ' Score ' } });
    fireEvent.keyDown(field, { key: 'Enter' });

    expect(onSubmit).toHaveBeenCalledWith('Score');
  });

  it('should commit a new name when it loses focus', () => {
    const { field, onSubmit } = renderInRow();
    fireEvent.change(field, { target: { value: 'Score' } });
    fireEvent.blur(field);

    expect(onSubmit).toHaveBeenCalledWith('Score');
  });

  it('should cancel, commit nothing and return focus to the row on Escape', () => {
    const { field, onSubmit, onCancel, row } = renderInRow();
    fireEvent.change(field, { target: { value: 'Score' } });
    fireEvent.keyDown(field, { key: 'Escape' });

    expect(onCancel).toHaveBeenCalled();
    expect(onSubmit).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(row);
  });

  it('should cancel when the name is unchanged or empty', () => {
    const { field, onSubmit, onCancel } = renderInRow();
    fireEvent.change(field, { target: { value: '  ' } });
    fireEvent.keyDown(field, { key: 'Enter' });

    expect(onSubmit).not.toHaveBeenCalled();
    expect(onCancel).toHaveBeenCalled();
  });

  it("should keep its keys from reaching the row's own shortcuts", () => {
    const onRowKey = vi.fn();
    render(
      <div
        role="button"
        tabIndex={0}
        onKeyDown={onRowKey}
      >
        <RenameField
          name="Hud"
          onSubmit={vi.fn()}
          onCancel={vi.fn()}
        />
      </div>,
    );
    fireEvent.keyDown(screen.getByLabelText('Rename Hud'), { key: 'Backspace' });

    expect(onRowKey).not.toHaveBeenCalled();
  });
});
