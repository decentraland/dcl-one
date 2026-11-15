import { useState } from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { TestProviders } from '../../../tests/utils/TestProviders';
import { FieldSelect, SelectOption } from './FieldSelect';

type FieldProps = { onChange: (value: string) => void; onRowPress: () => void };

function Field({ onChange, onRowPress }: FieldProps) {
  const [value, setValue] = useState('all');
  return (
    <TestProviders>
      <div onClick={onRowPress}>
        <FieldSelect
          aria-label="Parcels"
          value={value}
          onChange={next => {
            onChange(next);
            setValue(next);
          }}
        >
          <SelectOption value="all">All parcels</SelectOption>
          <SelectOption value="none">None</SelectOption>
        </FieldSelect>
      </div>
    </TestProviders>
  );
}

const combobox = () => screen.getByRole('combobox', { name: 'Parcels' });
/** The open list is modal: the element before it covers the page and takes the outside press. */
const backdrop = () => screen.getByRole('listbox').previousElementSibling as HTMLElement;

function openField() {
  const handlers = { onChange: vi.fn(), onRowPress: vi.fn() };
  render(<Field {...handlers} />);
  combobox().focus();
  fireEvent.click(combobox());
  handlers.onRowPress.mockClear();
  return handlers;
}

describe('FieldSelect', () => {
  afterEach(cleanup);

  it('a press outside the open list only closes it', () => {
    const { onChange, onRowPress } = openField();

    const focusMoves = fireEvent.mouseDown(backdrop());
    fireEvent.click(backdrop());

    expect(screen.queryByRole('listbox')).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
    expect(onRowPress).not.toHaveBeenCalled();
    expect(focusMoves).toBe(false);
    expect(document.activeElement).toBe(combobox());
  });

  it('the page behind does not scroll while the list is open', () => {
    openField();
    expect(document.body.style.overflow).toBe('hidden');

    fireEvent.click(screen.getByRole('option', { name: 'None' }));

    expect(screen.queryByRole('listbox')).toBeNull();
    expect(document.body.style.overflow).toBe('');
  });
});
