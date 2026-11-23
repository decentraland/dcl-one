import React from 'react';
import { Option } from '../Option';
import { MultipleOption } from '../MultipleOption';
import type { Props as OptionProp } from '../Option/types';
import type { Props } from './types';
import './SelectedOption.css';

const SelectedOption: React.FC<Props> = props => {
  const { minWidth, multiple, selectedValue, onRemove } = props;
  return (
    <div className="SelectedOptionContainer">
      {multiple ? (
        <MultipleOption
          className="SelectedOption"
          value={selectedValue as OptionProp[]}
          minWidth={minWidth}
          onRemove={onRemove}
        />
      ) : (
        <Option
          {...selectedValue}
          label={(selectedValue as OptionProp).selectedLabel ?? (selectedValue as OptionProp).label}
          className="SelectedOption"
          minWidth={minWidth}
        />
      )}
    </div>
  );
};

export default React.memo(SelectedOption);
