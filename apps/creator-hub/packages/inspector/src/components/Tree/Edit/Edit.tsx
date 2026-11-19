import { useCallback, useState } from 'react';

import { Modal } from '../../Modal';
import { Button } from '../../Button';
import { Input } from '../../Input';
import type { Props } from './types';

import './Edit.css';

export function Edit({ value, onCancel, onSubmit }: Props) {
  const [tmpValue, setTmpValue] = useState(value);
  const [isOpen, setIsOpen] = useState(false);

  const handleModalClose = useCallback(() => {
    setIsOpen(false);
    onCancel();
  }, []);

  const handleConfirm = useCallback(() => {
    setIsOpen(false);
    onSubmit(tmpValue);
  }, [tmpValue]);

  const handleChange = useCallback((value: string) => {
    setTmpValue(value);
  }, []);

  const handleBlur = useCallback(() => {
    setIsOpen(true);
  }, []);

  return (
    <>
      <Modal
        isOpen={isOpen && tmpValue !== value}
        onRequestClose={handleModalClose}
        className="EditTree"
      >
        <h2>⚠️ Rename</h2>
        <span>
          Do you want to rename "{value}" to "{tmpValue}"
        </span>
        <div>
          <Button
            variant="primary"
            size="lg"
            onClick={handleConfirm}
          >
            Yes
          </Button>
          <Button
            variant="secondary"
            size="lg"
            onClick={handleModalClose}
          >
            Cancel
          </Button>
        </div>
      </Modal>
      <Input
        value={value}
        onCancel={onCancel}
        onSubmit={onSubmit}
        onChange={handleChange}
        onBlur={handleBlur}
      />
    </>
  );
}
