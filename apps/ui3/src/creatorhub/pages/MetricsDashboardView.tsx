import Button from "../../atoms/Button";
import { PageHeader, SurfaceCard, SectionCard } from "../../components/Surface";
import MetricsNavigation from "../components/MetricsNavigation";
import EmptyState from "../../components/EmptyState";
import Spinner from "../../atoms/Spinner";
import DatumBadge from "../components/DatumBadge";
import DatumNote from "../components/DatumNote";
import { NO_VALUE, showable, type Datum } from "../lib/datum";
import "./metricsdashboardview.css";

const WalletGlyph = (
  <svg
    viewBox="0 0 24 24"
    width="34"
    height="34"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.6"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M3 7a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v1" />
    <path d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-6a2 2 0 0 0-2-2H5a2 2 0 0 1-2-2Z" />
    <circle cx="16.5" cy="13" r="1.2" fill="currentColor" stroke="none" />
  </svg>
);

const ChartGlyph = (
  <svg
    viewBox="0 0 24 24"
    width="34"
    height="34"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.6"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M4 20V10" />
    <path d="M10 20V4" />
    <path d="M16 20v-6" />
    <path d="M2 20h20" />
  </svg>
);

type CmSummaryCard = {
  label: string;
  value: string;
  note?: string;
  unavailable?: boolean;
  datum?: Datum<number | string>;
};

type CmSceneRow = {
  title: string;
  location: string;
  href: string | null;
  visits30d: number | null;
  liveNow: number | null;
};

type MetricsDashboardViewProps = {
  windowDays?: number;
  noAddress?: boolean;
  rescoping?: boolean;
  loadError?: boolean;
  retrying?: boolean;
  empty?: boolean;
  cards?: CmSummaryCard[] | null;
  sceneRows?: CmSceneRow[];
  scenesHref?: string;
  onConnect?: () => void;
  onRetry?: () => void;
};

export default function MetricsDashboardView({
  windowDays = 7,
  noAddress = true,
  rescoping = false,
  loadError = false,
  retrying = false,
  empty = false,
  cards = null,
  sceneRows = [],
  scenesHref = "/create/scene-analytics",
  onConnect,
  onRetry,
}: MetricsDashboardViewProps) {
  return (
    <div className="cm">
      <PageHeader title="Metrics" description={<>Your published collections, storefront sales (last {windowDays} days), and scene visits.</>} />
      <MetricsNavigation active="creator" scenesHref={scenesHref} />

      {noAddress ? (
        rescoping ? (
          <div
            className="cm__empty"
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: 14,
              padding: "56px 24px",
            }}
            role="status"
            aria-live="polite"
          >
            <Spinner aria-hidden="true" />
            <p style={{ margin: 0, color: "var(--ink-45)", fontSize: 14 }}>
              Loading your metrics&#x2026;
            </p>
          </div>
        ) : (
          <EmptyState
            className="cm__empty"
            icon={WalletGlyph}
            iconWash
            title="Sign in to see your creator metrics"
            subtitle="Your published collections, sales, and scene visits will appear here."
            actions={[{ label: "Sign in", onClick: onConnect }]}
            variant={undefined}
            tone={undefined}
            actionsGap={undefined}
            style={undefined}
          />
        )
      ) : (
        <>
          {loadError ? (
            <div className="cm__loaderror" role="alert">
              <span>Couldn&#x2019;t load your metrics right now.</span>
              <Button
                variant="secondary"
                size="sm"
                onClick={onRetry}
                disabled={retrying}
              >
                {retrying ? "Retrying\u{2026}" : "Try again"}
              </Button>
            </div>
          ) : empty ? (
            <EmptyState
              className="cm__empty"
              icon={ChartGlyph}
              iconWash
              title="Nothing published yet"
              subtitle={"You haven\u{2019}t published a collection or a scene, or listed an item with this wallet \u{2014} there\u{2019}s no performance data to show yet."}
              actions={[
                { label: "Create a collection", href: "/create/wearables" },
                {
                  label: "Build a scene",
                  href: "/create/scenes",
                  variant: "outline",
                },
              ]}
              variant={undefined}
              tone={undefined}
              actionsGap={undefined}
              style={undefined}
            />
          ) : (
            <div className="cm__cards">
              {(cards ?? []).map((c, i) => (
                <SummaryCard
                  key={i}
                  label={c.label}
                  value={c.value}
                  note={c.note}
                  unavailable={c.unavailable}
                  datum={c.datum}
                />
              ))}
            </div>
          )}

          {!loadError && !empty && sceneRows.length > 0 && (
            <SectionCard className="cm__scenes" title="Visits per scene (30d)" titleId="cm-scenes">
              <div className="cm__scroll" role="region" aria-label="Visits per scene" tabIndex={0}>
              <table className="cm__scenetable">
                <thead>
                  <tr>
                    <th scope="col">Scene</th>
                    <th scope="col">Location</th>
                    <th scope="col" className="cm__num">Visits (30d)</th>
                    <th scope="col" className="cm__num">In scene now</th>
                  </tr>
                </thead>
                <tbody>
                  {sceneRows.map((r, i) => (
                    <tr key={i}>
                      <td>
                        {r.href ? (
                          <a href={r.href} target="_blank" rel="noreferrer">
                            {r.title}
                          </a>
                        ) : (
                          r.title
                        )}
                      </td>
                      <td className="cm__loc">{r.location || "\u{2014}"}</td>
                      <td className="cm__num">
                        {r.visits30d === null ? (
                          <span className="cm__na">Not available</span>
                        ) : (
                          r.visits30d.toLocaleString("en-US")
                        )}
                      </td>
                      <td className="cm__num">
                        {r.liveNow === null ? (
                          <span className="cm__na">Not available</span>
                        ) : (
                          r.liveNow.toLocaleString("en-US")
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              </div>
            </SectionCard>
          )}

          </>
      )}
    </div>
  );
}

type SummaryCardProps = {
  label: string;
  value: string;
  note?: string;
  unavailable?: boolean;
  datum?: Datum<number | string>;
};

function SummaryCard({
  label,
  value,
  note,
  unavailable,
  datum,
}: SummaryCardProps) {
  const absent = datum ? !showable(datum) : Boolean(unavailable);
  const shown = datum && !showable(datum) ? NO_VALUE : value;

  return (
    <SurfaceCard className="cm__card">
      <div className="cm__cardlabelrow">
        <p className="cm__cardlabel">{label}</p>
        {datum ? <DatumBadge datum={datum} /> : null}
      </div>
      <div
        className="cm__cardvalue"
        style={
          absent
            ? { fontSize: 16, fontWeight: 600, color: "var(--ink-45)" }
            : undefined
        }
      >
        {shown}
      </div>
      {note && <p className="cm__cardnote">{note}</p>}
      {datum ? <DatumNote datum={datum} /> : null}
    </SurfaceCard>
  );
}
