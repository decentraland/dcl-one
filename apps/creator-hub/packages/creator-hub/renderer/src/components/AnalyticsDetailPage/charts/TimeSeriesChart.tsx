import { useId } from 'react';
import LineChart from '@ui/atoms/LineChart';
import Text from '@ui/atoms/Text';

import type { TimeSeriesPoint } from '/shared/types/place-analytics';

import { t } from '/@/modules/store/translation/utils';

import { ChartTooltipContent } from './ChartTooltipContent';
import { EndOfLineLabels, SettlingEndMarks } from './EndOfLineLabels';
import { SeriesGradient } from './SeriesGradient';
import {
  CHART_HEIGHT,
  CHART_MARGIN,
  FINAL_SERIES_ID,
  REFERENCE_HEADROOM,
  SERIES_COLOR,
  SETTLING_SERIES_ID,
  fortnightlyTicks,
  percentTicks,
  seriesPaint,
  tickDateFormat,
} from './theme';

import './styles.css';

type Props = {
  points: TimeSeriesPoint[];
  /** Names the series in the tooltip. The chart's own title is its label. */
  seriesName: string;
  formatValue: (value: number) => string;
  /** Upper bound of the y axis; the axis starts at 0. */
  maxValue?: number;
  /** Dashed benchmark drawn across the plot, e.g. the Day 7 target. Always kept in view. */
  reference?: { value: number; label: string };
};

export function TimeSeriesChart({ points, seriesName, formatValue, maxValue, reference }: Props) {
  const gradientId = `series-gradient-${useId().replace(/:/g, '')}`;

  if (points.length === 0) {
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

  const values = points.map(point => point.value);
  const dataMax = Math.max(...values.map(value => value ?? 0));
  const yMax =
    maxValue ?? (reference ? Math.max(dataMax, reference.value * REFERENCE_HEADROOM) : undefined);
  const lastPoint = points.findLast(point => point.value !== null);
  const lastFinalIndex = points.findLastIndex(point => !point.settling && point.value !== null);
  const lastFinal = points[lastFinalIndex];
  const hasSettling = points.some(point => point.settling);
  const paint = seriesPaint(gradientId);
  const formatPoint = (value: number | null) => (value === null ? '-' : formatValue(value));
  const dates = points.map(point => point.date);
  const line = { label: seriesName, color: SERIES_COLOR, paint };

  return (
    <>
      <LineChart
        className="AnalyticsChart"
        aria-label={seriesName}
        dates={dates}
        height={CHART_HEIGHT}
        margin={CHART_MARGIN}
        yMin={0}
        yMax={yMax}
        yTicks={percentTicks(maxValue)}
        yTickCount={5}
        xTicks={fortnightlyTicks(dates)}
        formatX={date => tickDateFormat.format(date)}
        formatY={formatValue}
        reference={reference}
        series={[
          ...(hasSettling
            ? [
                {
                  ...line,
                  id: SETTLING_SERIES_ID,
                  dash: '0 6',
                  mark: 'hollow' as const,
                  data: points.map((point, index) =>
                    point.settling || index === lastFinalIndex ? point.value : null,
                  ),
                },
              ]
            : []),
          {
            ...line,
            id: FINAL_SERIES_ID,
            data: points.map(point => (point.settling ? null : point.value)),
          },
        ]}
        renderTooltip={hover => (
          <ChartTooltipContent
            {...hover}
            formatValue={formatPoint}
          />
        )}
      >
        {scales => (
          <>
            <SeriesGradient
              id={gradientId}
              values={values}
              scales={scales}
            />
            {lastPoint?.value != null &&
              (lastPoint.settling ? (
                <SettlingEndMarks
                  end={{ date: lastPoint.date, value: lastPoint.value }}
                  lastFinal={
                    lastFinal?.value != null
                      ? {
                          date: lastFinal.date,
                          value: lastFinal.value,
                          text: formatValue(lastFinal.value),
                        }
                      : undefined
                  }
                  paint={paint}
                  scales={scales}
                />
              ) : (
                <EndOfLineLabels
                  labels={[
                    {
                      key: seriesName,
                      date: lastPoint.date,
                      value: lastPoint.value,
                      color: paint,
                      text: formatValue(lastPoint.value),
                    },
                  ]}
                  scales={scales}
                />
              ))}
          </>
        )}
      </LineChart>
      {hasSettling && (
        <div className="ChartCaption">
          <span className="ChartSwatch Dotted" />
          <Text
            variant="body2"
            className="ChartCaptionText"
          >
            {t('analytics.detail.retention.day_7_cohort.settling.caption')}
          </Text>
        </div>
      )}
    </>
  );
}
