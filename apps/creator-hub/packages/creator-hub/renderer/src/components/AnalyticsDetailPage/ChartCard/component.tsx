import type { ReactNode } from 'react';
import Text from '@ui/atoms/Text';

import './styles.css';

type Props = {
  title: string;
  description?: string;
  children: ReactNode;
};

/** A titled panel wrapping one chart or stat block. */
export function ChartCard({ title, description, children }: Props) {
  return (
    <div className="ChartCard">
      <div className="ChartCardHeader">
        <Text
          variant="h6"
          className="Title"
        >
          {title}
        </Text>
        {description && (
          <Text
            variant="body2"
            className="Description"
          >
            {description}
          </Text>
        )}
      </div>
      <div className="ChartCardBody">{children}</div>
    </div>
  );
}
