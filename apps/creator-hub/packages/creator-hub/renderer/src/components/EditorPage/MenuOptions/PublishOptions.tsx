import Text from '@ui/atoms/Text';

import type { PublishOptionsProps } from './types';

export function PublishOptions({ options }: PublishOptionsProps) {
  return (
    <div className="PublishOptions">
      {options.map(option => (
        <button
          key={option.id}
          type="button"
          className="option"
          onClick={option.action}
        >
          <Text
            as="span"
            variant="body1"
          >
            {option.label}
          </Text>
        </button>
      ))}
    </div>
  );
}
