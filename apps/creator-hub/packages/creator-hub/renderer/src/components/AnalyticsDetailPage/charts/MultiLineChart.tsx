import { useId } from 'react';
import LineChart from '@ui/atoms/LineChart';
import Text from '@ui/atoms/Text';

import type { TimeSeriesPoint } from '/shared/types/place-analytics';

import { t } from '/@/modules/store/translation/utils';

import { ChartTooltipContent } from './ChartTooltipContent';
import { EndOfLineLabels } from './EndOfLineLabels';
import { SeriesGradient } from './SeriesGradient';
import {
  CATEGORICAL_COLORS,
  CHART_HEIGHT,
  CHART_MARGIN,
  SERIES_COLOR,
  fortnightlyTicks,
  seriesPaint,
  tickDateFormat,
} from './theme';

import './styles.css';

export type LineSeries = {
  key: string;
  label: string;
  points: TimeSeriesPoint[];
};

type Props = {
  /** Rendered in fixed order, so each series keeps its color across charts. A lone series wears the brand gradient. */
  series: LineSeries[];
  formatValue: (value: number) => string;
  maxValue?: number;
  height?: number;
};

/** Several series over the same weeks, each labelled at its last value; the caller renders the legend. */
export function MultiLineChart({ series, formatValue, maxValue, height }: Props) {
  const isSingle = series.length === 1;
  const id = `series-gradient-${useId().replace(/:/g, '')}`;
  const gradientId = isSingle ? id : undefined;
  const dates = series[0]?.points.map(point => point.date) ?? [];

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

  const colorOf = (index: number) =>
    isSingle ? SERIES_COLOR : CATEGORICAL_COLORS[index % CATEGORICAL_COLORS.length];

  const endLabels = series
    .map((entry, index) => {
      const lastPoint = [...entry.points].reverse().find(point => point.value !== null);
      return lastPoint?.value == null
        ? null
        : {
            key: entry.key,
            date: lastPoint.date,
            value: lastPoint.value,
            color: isSingle ? seriesPaint(gradientId) : colorOf(index),
            text: formatValue(lastPoint.value),
          };
    })
    .filter(label => label !== null);

  const formatPoint = (value: number | null) => (value === null ? '-' : formatValue(value));

  return (
    <LineChart
      className="AnalyticsChart"
      aria-label={series.map(entry => entry.label).join(', ')}
      dates={dates}
      height={height ?? CHART_HEIGHT}
      margin={CHART_MARGIN}
      yMin={0}
      yMax={maxValue}
      yTickCount={5}
      xTicks={fortnightlyTicks(dates)}
      formatX={date => tickDateFormat.format(date)}
      formatY={formatValue}
      series={series.map((entry, index) => ({
        id: entry.key,
        label: entry.label,
        color: colorOf(index),
        paint: isSingle ? seriesPaint(gradientId) : undefined,
        data: entry.points.map(point => point.value),
      }))}
      renderTooltip={hover => (
        <ChartTooltipContent
          {...hover}
          formatValue={formatPoint}
        />
      )}
    >
      {scales => (
        <>
          {gradientId && (
            <SeriesGradient
              id={gradientId}
              values={series[0].points.map(point => point.value)}
              scales={scales}
            />
          )}
          <EndOfLineLabels
            labels={endLabels}
            scales={scales}
          />
        </>
      )}
    </LineChart>
  );
}
