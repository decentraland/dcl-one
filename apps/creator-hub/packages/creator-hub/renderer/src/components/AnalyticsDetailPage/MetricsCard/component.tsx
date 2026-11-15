import { type Metric, MetricDisplay } from '../MetricDisplay';

import './styles.css';

type Props = {
  /** A full-width first row, rendered like any other metric. */
  headline?: Metric;
  /** Laid out two per row. */
  metrics: Metric[];
};

export function MetricsCard({ headline, metrics }: Props) {
  return (
    <div className="MetricsCard">
      {headline && <MetricDisplay {...headline} />}
      <div className="MetricsGrid">
        {metrics.map(metric => (
          <MetricDisplay
            key={metric.label}
            {...metric}
          />
        ))}
      </div>
    </div>
  );
}
