import UiStackedBarChart from '@ui/atoms/StackedBarChart';
import Text from '@ui/atoms/Text';

import { t } from '/@/modules/store/translation/utils';

import {
  CATEGORICAL_COLORS,
  CHART_HEIGHT,
  CHART_MARGIN,
  tickDateFormat,
  tooltipDateFormat,
} from './theme';

import './styles.css';

export type StackedSeries = {
  key: string;
  label: string;
  /** One value per week, aligned with `dates`. */
  values: Array<number | null>;
};

type Props = {
  /** Epoch milliseconds, one per bar. */
  dates: number[];
  series: StackedSeries[];
  formatValue: (value: number) => string;
};

/**
 * Weekly composition. Segments are separated by a 2px surface-colored gap
 * rather than an outline, and the whole bar carries one tooltip listing every
 * series, so the reader never has to hit a single segment.
 */
export function StackedBarChart({ dates, series, formatValue }: Props) {
  if (dates.length === 0) {
    return (
      <div className="ChartEmpty">
        <Text
          variant="body2"
          className="ChartEmptyText"
        >
          {t('analytics.detail.charts.no_data')}
        </Text>
      </div>
    );
  }

  return (
    <UiStackedBarChart
      className="AnalyticsChart StackedBars"
      aria-label={series.map(entry => entry.label).join(', ')}
      dates={dates}
      height={CHART_HEIGHT}
      margin={CHART_MARGIN}
      // Leftover band space is air: holds bars at the 24px cap instead of filling the slot.
      gapRatio={0.56}
      radius={4}
      // A label per weekly bar crowds the axis; label every other one.
      xTickFilter={(_date, index) => index % 2 === 0}
      formatX={date => tickDateFormat.format(date)}
      formatDate={date => tooltipDateFormat.format(date)}
      formatY={formatValue}
      series={series.map((entry, index) => ({
        id: entry.key,
        label: entry.label,
        color: CATEGORICAL_COLORS[index % CATEGORICAL_COLORS.length],
        data: entry.values,
      }))}
    />
  );
}
