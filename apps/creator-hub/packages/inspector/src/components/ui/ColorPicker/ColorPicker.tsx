import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { cx } from '../../../lib/utils/cx';

import { TextField } from '../TextField';

import { type Props } from './types';

import './ColorPicker.css';

const ColorPicker: React.FC<Props> = ({
  className,
  label,
  disabled,
  value = '#FFFFFF',
  error,
  onChange,
}) => {
  const [color, setColor] = useState<string>(value.toString());
  const inputRef = useRef<HTMLInputElement>(null);

  const [isPicking, setPicking] = useState(false);
  const lastValueRef = useRef(value);

  // React's onChange fires on every move; the native `change` event fires once, when the picker
  // closes on a final colour.
  useEffect(() => {
    const input = inputRef.current;
    if (!input) return;
    const endPicking = () => setPicking(false);
    input.addEventListener('change', endPicking);
    return () => input.removeEventListener('change', endPicking);
  }, []);

  // The value can change from outside (undo, another entity selected). It is synced only while the
  // colour isn't being dragged, so a lagging engine round-trip can't snap the colour back mid-drag
  // (docs/coding-standards.md, "Don't mirror props into local state via useEffect").
  useEffect(() => {
    const externalChange = value !== lastValueRef.current;
    lastValueRef.current = value;
    if (isPicking) return;
    if (externalChange) setColor(value.toString());
  }, [value, isPicking]);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      e.preventDefault();
      e.stopPropagation();
      // React also reports the closing `change` here, after the native listener ended the drag
      if (e.nativeEvent.type === 'input') setPicking(true);
      setColor(e.target.value.toUpperCase());
      onChange && onChange(e);
    },
    [setColor, onChange],
  );

  const handleClick = useCallback(() => {
    inputRef.current?.click();
  }, [inputRef]);

  const renderLeftContent = useMemo(() => {
    return (
      <div
        className="ColorPreview"
        style={{ backgroundColor: color }}
      />
    );
  }, [color]);

  return (
    <div className={cx('ColorPickerContainer', className, { error, disabled })}>
      <TextField
        className="ColorPickerTextField"
        label={label}
        placeholder="Pick a Color"
        onClick={handleClick}
        value={color}
        error={error}
        disabled={disabled}
        leftContent={renderLeftContent}
        readOnly
      />
      <input
        className="ColorPickerInput"
        type="color"
        ref={inputRef}
        value={color}
        onChange={handleChange}
      />
    </div>
  );
};

export default React.memo(ColorPicker);
