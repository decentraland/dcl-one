import type { PlaceEngagementMetrics } from '/shared/types/place-analytics';

import { t } from '/@/modules/store/translation/utils';

import { formatDecimal, formatPercentage } from '../../AnalyticsPage/utils';
import { ChartCard } from '../ChartCard';
import { HeroStat } from '../HeroStat';
import { TabHeading } from '../TabHeading';
import { MultiLineChart } from '../charts';

import './styles.css';

const SOCIAL_CHART_HEIGHT = 260;

type Props = {
  engagement: PlaceEngagementMetrics;
};

/**
 * A single trailing-window figure.
 *
 * There is no weekly playtime metric, so these carry no series and no
 * week-over-week delta — only the window the user selected.
 */
function ScalarCard({
  title,
  description,
  value,
}: {
  title: string;
  description: string;
  value: number | null;
}) {
  return (
    <ChartCard
      title={title}
      description={description}
    >
      <HeroStat
        value={formatDecimal(value)}
        unit={value === null ? undefined : t('analytics.detail.engagement.minutes')}
      />
    </ChartCard>
  );
}

export function EngagementTab({ engagement }: Props) {
  return (
    <div className="EngagementTab">
      <TabHeading
        title={t('analytics.detail.engagement.title')}
        description={t('analytics.detail.engagement.description')}
      />
      <div className="Cards">
        <div className="PlaytimeCards">
          <ScalarCard
            title={t('analytics.detail.engagement.playtime.title')}
            description={t('analytics.detail.engagement.playtime.description')}
            value={engagement.medianPlaytime}
          />
          <ScalarCard
            title={t('analytics.detail.engagement.afk.title')}
            description={t('analytics.detail.engagement.afk.description')}
            value={engagement.afkTime}
          />
        </div>
        <ChartCard
          title={t('analytics.detail.engagement.social.title')}
          description={t('analytics.detail.engagement.social.description')}
        >
          <MultiLineChart
            series={[
              {
                key: 'sociallyEngaged',
                label: t('analytics.detail.engagement.social.engaged.label'),
                points: engagement.sociallyEngaged,
              },
            ]}
            formatValue={formatPercentage}
            maxValue={100}
            height={SOCIAL_CHART_HEIGHT}
          />
        </ChartCard>
        {/*
          Social interactions by action (messages, emotes, friendships), as both
          weekly totals and per-visitor rates: awaiting per-action metrics.
          `socially_engaged_ratio_weekly` is one ratio with no breakdown, drawn by
          the chart above. Friendships have never had a metric.

        <ChartCard
          title={t('analytics.detail.engagement.social.title')}
          description={t('analytics.detail.engagement.social.description')}
        >
          <ChartLegend
            entries={[
              {
                label: t('analytics.detail.engagement.social.messages.label'),
                color: CATEGORICAL_COLORS[0],
                tooltip: t('analytics.detail.engagement.social.messages.tooltip'),
              },
              {
                label: t('analytics.detail.engagement.social.emotes.label'),
                color: CATEGORICAL_COLORS[1],
                tooltip: t('analytics.detail.engagement.social.emotes.tooltip'),
              },
              {
                label: t('analytics.detail.engagement.social.friendships.label'),
                color: CATEGORICAL_COLORS[2],
                tooltip: t('analytics.detail.engagement.social.friendships.tooltip'),
              },
            ]}
          />
          <div className="SocialCharts">
            <div className="SocialChart">
              <Text variant="subtitle1" className="ChartTitle">
                {t('analytics.detail.engagement.social.totals_title')}
              </Text>
              <MultiLineChart
                series={socialSeries('weeklyTotals')}
                formatValue={formatCount}
                height={SOCIAL_CHART_HEIGHT}
                compact
              />
            </div>
            <div className="SocialChart">
              <Text variant="subtitle1" className="ChartTitle">
                {t('analytics.detail.engagement.social.rate_title')}
              </Text>
              <MultiLineChart
                series={socialSeries('visitorRate')}
                formatValue={formatPercentage}
                maxValue={100}
                height={SOCIAL_CHART_HEIGHT}
                compact
              />
            </div>
          </div>
        </ChartCard>
        */}
      </div>
    </div>
  );
}
