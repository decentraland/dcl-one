import type { ReactNode } from 'react';
import IconButton from '@ui/atoms/IconButton';
import Text from '@ui/atoms/Text';
import Tooltip from '@ui/atoms/Tooltip';

import { InfoOutlinedIcon } from '@ui/atoms/icons-material';

import './styles.css';

export type Metric = {
  label: string;
  /** Explains how the metric is computed. Shown on the label's info button. */
  tooltip: string;
  value: ReactNode;
  /** Set beside the value at regular weight, e.g. "min". */
  unit?: string;
  /** Palette path for the value, e.g. "success.main", or a CSS color. */
  color?: string;
};

const PALETTE_COLORS: Record<string, string> = {
  'success.main': 'var(--success)',
  'primary.main': 'var(--primary)',
};

/** A labelled figure: the label with its info tooltip, then the value. */
export function MetricDisplay({ label, tooltip, value, unit, color }: Metric) {
  return (
    <div className="MetricDisplay">
      <div className="MetricLabel">
        <Text
          variant="body2"
          className="Label"
        >
          {label}
        </Text>
        <Tooltip
          label={tooltip}
          side="top"
          arrow
          portal
          keepInViewport
          snap
          maxWidth={300}
          tipClassName="MetricTooltip"
        >
          <IconButton
            className="MetricInfo"
            aria-label={tooltip}
          >
            <InfoOutlinedIcon />
          </IconButton>
        </Tooltip>
      </div>
      <Text
        variant="h4"
        as="p"
        className="MetricValue"
        style={{
          color: color ? (PALETTE_COLORS[color] ?? color) : 'var(--palette-neutral-softWhite)',
        }}
      >
        {value}
        {unit && <span className="MetricUnit"> {unit}</span>}
      </Text>
    </div>
  );
}
