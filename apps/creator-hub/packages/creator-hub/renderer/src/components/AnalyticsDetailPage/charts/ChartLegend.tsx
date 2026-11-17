import Text from '@ui/atoms/Text';
import Tooltip from '@ui/atoms/Tooltip';

import { InfoOutlinedIcon } from '@ui/atoms/icons-material';

import './styles.css';

export type LegendEntry = {
  label: string;
  color: string;
  /** Explains what the series counts. */
  tooltip?: string;
};

type Props = {
  entries: LegendEntry[];
};

/**
 * Identity for multi-series charts. Always rendered when there is more than one
 * series, so identity never depends on color alone.
 */
export function ChartLegend({ entries }: Props) {
  return (
    <div className="ChartLegend">
      {entries.map(entry => (
        <div
          className="LegendEntry"
          key={entry.label}
        >
          <div
            className="LegendKey"
            style={{ backgroundColor: entry.color }}
          />
          <Text variant="body2">{entry.label}</Text>
          {entry.tooltip && (
            <Tooltip
              label={entry.tooltip}
              side="top"
              arrow
              portal
              keepInViewport
              maxWidth={300}
            >
              <InfoOutlinedIcon
                fontSize="small"
                tabIndex={0}
                aria-label={entry.tooltip}
              />
            </Tooltip>
          )}
        </div>
      ))}
    </div>
  );
}
