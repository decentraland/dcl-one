import type { LineChartHover } from '@ui/atoms/LineChart';

import { t } from '/@/modules/store/translation/utils';

import { FINAL_SERIES_ID, SETTLING_SERIES_ID, tooltipDateFormat } from './theme';

type Props = LineChartHover & {
  formatValue: (value: number | null) => string;
};

/** Axis tooltip for a week: the date, then a value, swatch and name per series, and a note on a settling cohort. */
export function ChartTooltipContent({ date, index, series, formatValue }: Props) {
  const rows = series.map(entry => ({ entry, value: entry.data[index] ?? null }));
  const finalValue = rows.find(row => row.entry.id === FINAL_SERIES_ID)?.value ?? null;
  const isSettling =
    finalValue === null &&
    rows.some(row => row.entry.id === SETTLING_SERIES_ID && row.value !== null);
  const visible = rows.filter(
    row => row.entry.id !== (isSettling ? FINAL_SERIES_ID : SETTLING_SERIES_ID),
  );
  const swatch =
    visible.length > 1 ? 'ChartSwatch' : `ChartSwatch ${isSettling ? 'Dotted' : 'Gradient'}`;

  return (
    <div className="ChartTooltip">
      <span className="ChartTooltipDate">{tooltipDateFormat.format(date)}</span>
      {visible.map(({ entry, value }) => (
        <div
          className="ChartTooltipRow"
          key={entry.id}
        >
          <span className="ChartTooltipValue">{formatValue(value)}</span>
          <span className="ChartTooltipSeries">
            <span
              className={swatch}
              style={visible.length > 1 ? { background: entry.color } : undefined}
            />
            {entry.label}
          </span>
        </div>
      ))}
      {isSettling && (
        <span className="ChartTooltipNote">
          {t('analytics.detail.retention.day_7_cohort.settling.tooltip')}
        </span>
      )}
    </div>
  );
}
