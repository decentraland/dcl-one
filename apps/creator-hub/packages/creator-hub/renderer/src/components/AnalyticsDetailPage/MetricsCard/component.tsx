import { Box } from 'decentraland-ui2';

import { type Metric, MetricDisplay } from '../MetricDisplay';

import './styles.css';

type Props = {
  /** Laid out two per row. */
  metrics: Metric[];
};

export function MetricsCard({ metrics }: Props) {
  return (
    <Box className="MetricsCard">
      <Box className="MetricsGrid">
        {metrics.map(metric => (
          <MetricDisplay
            key={metric.label}
            {...metric}
          />
        ))}
      </Box>
    </Box>
  );
}
