import { type Metric, MetricDisplay } from '../MetricDisplay';

import './styles.css';

type Props = {
  /** Laid out two per row. */
  metrics: Metric[];
};

export function MetricsCard({ metrics }: Props) {
  return (
    <div className="MetricsCard">
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
