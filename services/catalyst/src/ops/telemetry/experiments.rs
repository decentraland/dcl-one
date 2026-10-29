//! The experiments readouts over Segment events carrying
//! `properties.exp_key`: `GET /dash/experiments` (the aggregate the sites
//! poll every ~30 s, memoised 60 s and served by the partial index
//! `events_experiments_idx`), `/readout` (per-variant exposures and
//! conversions with the pooled two-proportion z-test and the Beta(1,1)
//! posterior, formulas identical to the sites CLI's), `/timeseries` and
//! `/rates` (one scoped scan, both shapes memoised together).

use std::collections::BTreeMap;
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{SharedState, dashboard, db_err};

const SYNTHETIC_PREFIXES: [&str; 4] = [
    "readout_probe_",
    "verify_readout_",
    "verify_throwaway",
    "pm_smoke",
];

/// How long one `GET /experiments` aggregate is served from memory. The route
/// is polled every ~30 s (5,992 calls in 50 h), each a scan of every segment
/// event; a 60 s memo caps that at one scan per minute per process, however
/// many pollers there are.
pub const LIST_TTL: Duration = Duration::from_secs(60);

/// Predicate and grouped expressions match `events_experiments_idx` in
/// `schemas/60_ops.sql` verbatim so the partial index serves the scan.
pub const LIST_SQL: &str = "SELECT e.body->'properties'->>'exp_key' AS exp_key, \
            e.body->'properties'->>'variant' AS variant, \
            e.body->>'event' AS event, \
            count(*) AS n \
     FROM telemetry.events e \
     WHERE e.source = 'segment' \
       AND e.body->'properties'->>'exp_key' IS NOT NULL \
     GROUP BY 1, 2, 3";

type ListRow = (String, Option<String>, Option<String>, i64);

#[derive(Deserialize)]
pub struct ListQuery {
    key: Option<String>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    fresh: Option<String>,
}

#[derive(Default)]
struct ExpAgg {
    exposures: i64,
    variants: Vec<String>,
    metrics: BTreeMap<String, i64>,
}

pub async fn list(
    State(st): State<SharedState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if q.key.as_deref().is_some_and(|k| !k.is_empty()) {
        return dashboard::experiments_get(
            State(st),
            Query(dashboard::ExperimentsQuery {
                key: q.key,
                user: q.user,
            }),
        )
        .await;
    }
    let pool = &st.pool;
    let body = st
        .experiments_cache
        .get_or_refresh(is_fresh(q.fresh.as_deref()), move || async move {
            sqlx::query_as::<_, ListRow>(LIST_SQL)
                .fetch_all(pool)
                .await
                .map(aggregate)
        })
        .await
        .map_err(|e| db_err("telemetry experiments", e))?;
    Ok(Json(body))
}

/// `fresh=1` (or `true`) turns the memo off for that call: the reload is
/// still single-flight and refreshes the slot for everyone else.
fn is_fresh(fresh: Option<&str>) -> bool {
    matches!(fresh, Some("1" | "true"))
}

fn aggregate(rows: Vec<ListRow>) -> Value {
    let mut agg: BTreeMap<String, ExpAgg> = BTreeMap::new();
    for (exp_key, variant, event, n) in rows {
        let a = agg.entry(exp_key).or_default();
        if let Some(v) = variant
            && !a.variants.contains(&v)
        {
            a.variants.push(v);
        }
        match event.as_deref() {
            Some("experiment_exposed") => a.exposures += n,
            Some(ev) => *a.metrics.entry(ev.to_string()).or_insert(0) += n,
            None => {}
        }
    }

    let mut experiments = Vec::new();
    let mut unreadable = Vec::new();
    for (exp_key, mut a) in agg {
        a.variants.sort();
        let reason = if SYNTHETIC_PREFIXES.iter().any(|p| exp_key.starts_with(p)) {
            Some("synthetic key (matches exclusion pattern)")
        } else if a.exposures == 0 {
            Some("no experiment_exposed events")
        } else if a.variants.len() < 2 {
            Some("fewer than 2 variants")
        } else {
            None
        };
        let entry = json!({
            "exp_key": exp_key,
            "exposures": a.exposures,
            "variants": a.variants,
            "metrics": a.metrics.iter()
                .map(|(event, count)| json!({ "event": event, "count": count }))
                .collect::<Vec<_>>(),
        });
        match reason {
            Some(r) => {
                let mut e = entry;
                e["reason"] = json!(r);
                unreadable.push(e);
            }
            None => {
                let control = if a.variants.iter().any(|v| v == "control") {
                    "control".to_string()
                } else {
                    a.variants[0].clone()
                };
                let mut e = entry;
                e["control"] = json!(control);
                experiments.push(e);
            }
        }
    }
    json!({ "experiments": experiments, "unreadable": unreadable })
}

/// The pooled two-proportion z-test and the Beta(1,1) posterior of
/// the v1 stack's `metabase/functions.sql` (`ext.norm_cdf`, `two_prop_z`,
/// `two_prop_p`, `beta_post_mean`, `beta_post_var`, `p_beats`), themselves
/// direct ports of `catalyrst/sites/scripts/story-readout.ts`, so the
/// console, the sites CLI and the Metabase cards compute the same numbers.
pub mod stats {
    /// Standard normal CDF (Abramowitz & Stegun 7.1.26).
    pub fn norm_cdf(z: f64) -> f64 {
        let t = 1.0 / (1.0 + 0.2316419 * z.abs());
        let d = 0.3989422804014327 * (-(z * z) / 2.0).exp();
        let p = d
            * t
            * (0.31938153
                + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
        if z >= 0.0 { 1.0 - p } else { p }
    }

    /// Pooled two-proportion z statistic (treatment - control); an empty arm
    /// or a zero standard error answers 0.
    pub fn two_prop_z(x1: f64, n1: f64, x2: f64, n2: f64) -> f64 {
        if n1 <= 0.0 || n2 <= 0.0 {
            return 0.0;
        }
        let pooled = (x1 + x2) / (n1 + n2);
        let se = (pooled * (1.0 - pooled) * (1.0 / n1 + 1.0 / n2)).sqrt();
        if !se.is_finite() || se <= 0.0 {
            return 0.0;
        }
        (x2 / n2 - x1 / n1) / se
    }

    /// Two-sided p-value of [`two_prop_z`]; no signal answers 1.
    pub fn two_prop_p(x1: f64, n1: f64, x2: f64, n2: f64) -> f64 {
        if n1 <= 0.0 || n2 <= 0.0 {
            return 1.0;
        }
        let z = two_prop_z(x1, n1, x2, n2);
        if z == 0.0 {
            return 1.0;
        }
        2.0 * (1.0 - norm_cdf(z.abs()))
    }

    /// Beta(1,1)-prior posterior mean = (s+1)/((s+1)+(n-s+1)).
    pub fn beta_post_mean(successes: f64, exposures: f64) -> f64 {
        (successes + 1.0) / ((successes + 1.0) + ((exposures - successes) + 1.0))
    }

    /// Beta(1,1)-prior posterior variance = a*b/((a+b)^2*(a+b+1)).
    pub fn beta_post_var(successes: f64, exposures: f64) -> f64 {
        let a = successes + 1.0;
        let b = (exposures - successes) + 1.0;
        (a * b) / ((a + b) * (a + b) * (a + b + 1.0))
    }

    /// P(treatment beats control), a normal approximation to the two Beta posteriors.
    pub fn p_beats(succ_c: f64, exp_c: f64, succ_t: f64, exp_t: f64) -> f64 {
        let spread = (beta_post_var(succ_t, exp_t) + beta_post_var(succ_c, exp_c)).sqrt();
        norm_cdf((beta_post_mean(succ_t, exp_t) - beta_post_mean(succ_c, exp_c)) / spread)
    }
}

#[derive(Deserialize)]
pub struct ReadoutQuery {
    exp_key: String,
    metric: String,
    control: String,
    alpha: Option<f64>,
    min_sample: Option<f64>,
    #[serde(default)]
    fresh: Option<String>,
}

/// `(exp_key, metric, control, alpha bits, min_sample bits)`.
pub type ReadoutKey = (String, String, String, Option<u64>, Option<u64>);

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ReadoutRow {
    pub variant: String,
    pub n_exposures: i64,
    pub successes: i64,
    pub rate: f64,
    pub control_rate: f64,
    pub diff: f64,
    pub z: f64,
    pub p_value: f64,
    pub significant: bool,
    pub bayes_mean: f64,
    pub bayes_control_mean: f64,
    pub bayes_ci_low: f64,
    pub bayes_ci_high: f64,
    pub p_beats_control: f64,
    pub verdict: String,
}

/// Per-variant distinct users exposed and converted; `metric` ending in
/// `_rate` names its event without the suffix.
const ARMS_SQL: &str = r#"
WITH cfg AS (
  SELECT $1::text AS exp_key,
         CASE WHEN $2::text LIKE '%\_rate' THEN left($2::text, length($2::text) - 5) ELSE $2::text END AS num_event
),
scoped AS (
  SELECT e.body->'properties'->>'variant' AS variant,
         e.body->>'event' AS event,
         COALESCE(e.body->'user'->>'id', e.body->'user'->>'username', e.body->>'userId', e.body->>'anonymousId') AS user_key
  FROM telemetry.events e, cfg
  WHERE e.source = 'segment'
    AND e.body->'properties'->>'exp_key' = cfg.exp_key
    AND e.body->'properties'->>'variant' IS NOT NULL
    AND COALESCE(e.body->'user'->>'id', e.body->'user'->>'username', e.body->>'userId', e.body->>'anonymousId') IS NOT NULL
)
SELECT s.variant,
       count(DISTINCT s.user_key) FILTER (WHERE s.event = 'experiment_exposed') AS n_exposures,
       count(DISTINCT s.user_key) FILTER (WHERE s.event = (SELECT num_event FROM cfg)) AS successes
FROM scoped s
GROUP BY s.variant
ORDER BY s.variant
"#;

pub type Arm = (String, i64, i64);

/// The old `READOUT_SQL`'s projection over the arms: every variant but the
/// control against the control arm; no control arm, no rows.
pub fn readout_rows(
    arms: &[Arm],
    control: &str,
    alpha: Option<f64>,
    min_sample: Option<f64>,
) -> Vec<ReadoutRow> {
    let alpha = alpha.unwrap_or(0.05);
    let min_sample = min_sample.unwrap_or(0.0);
    let Some((_, c_n, c_x)) = arms.iter().find(|(v, _, _)| v == control) else {
        return Vec::new();
    };
    let (c_n, c_x) = (*c_n as f64, *c_x as f64);
    let control_rate = if c_n > 0.0 { c_x / c_n } else { 0.0 };
    arms.iter()
        .filter(|(v, _, _)| v != control)
        .map(|(variant, n, x)| {
            let (nf, xf) = (*n as f64, *x as f64);
            let rate = if nf > 0.0 { xf / nf } else { 0.0 };
            let p_value = stats::two_prop_p(c_x, c_n, xf, nf);
            let mean = stats::beta_post_mean(xf, nf);
            let var = stats::beta_post_var(xf, nf);
            let lift = if nf > 0.0 && c_n > 0.0 {
                Some(xf / nf - c_x / c_n)
            } else {
                None
            };
            let verdict = match lift {
                Some(l) if p_value < alpha && l > 0.0 && nf.min(c_n) >= min_sample => "SHIP",
                Some(l) if p_value < alpha && l < 0.0 => "KILL",
                _ => "KEEP RUNNING",
            };
            ReadoutRow {
                variant: variant.clone(),
                n_exposures: *n,
                successes: *x,
                rate,
                control_rate,
                diff: rate - control_rate,
                z: stats::two_prop_z(c_x, c_n, xf, nf),
                p_value,
                significant: p_value < alpha,
                bayes_mean: mean,
                bayes_control_mean: stats::beta_post_mean(c_x, c_n),
                bayes_ci_low: (mean - 1.96 * var.sqrt()).max(0.0),
                bayes_ci_high: (mean + 1.96 * var.sqrt()).min(1.0),
                p_beats_control: stats::p_beats(c_x, c_n, xf, nf),
                verdict: verdict.to_string(),
            }
        })
        .collect()
}

pub async fn readout(
    State(st): State<SharedState>,
    Query(p): Query<ReadoutQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let key: ReadoutKey = (
        p.exp_key.clone(),
        p.metric.clone(),
        p.control.clone(),
        p.alpha.map(f64::to_bits),
        p.min_sample.map(f64::to_bits),
    );
    if is_fresh(p.fresh.as_deref()) {
        st.readout_cache.clear();
    }
    let pool = &st.pool;
    let rows = st
        .readout_cache
        .get_or_fetch(key, || async {
            let arms: Vec<Arm> = sqlx::query_as(ARMS_SQL)
                .bind(&p.exp_key)
                .bind(&p.metric)
                .fetch_all(pool)
                .await?;
            Ok::<_, sqlx::Error>(readout_rows(&arms, &p.control, p.alpha, p.min_sample))
        })
        .await
        .map_err(|e| db_err("telemetry experiments", e))?;
    Ok(Json(json!({
        "exp_key": p.exp_key,
        "metric": p.metric,
        "control": p.control,
        "alpha": p.alpha.unwrap_or(0.05),
        "min_sample": p.min_sample.unwrap_or(0.0),
        "rows": rows,
    })))
}

#[derive(Deserialize)]
pub struct SeriesQuery {
    exp_key: String,
    metric: String,
    #[serde(default)]
    fresh: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct TimeseriesRow {
    day: String,
    variant: String,
    exposures: i64,
    conversions: i64,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct RateRow {
    variant: String,
    exposures: i64,
    successes: i64,
    rate: f64,
}

/// `/timeseries` and `/rates` read the same scoped scan, so one statement computes both and
/// the memo serves whichever the dashboard asks for second.
#[derive(Clone)]
pub struct SeriesBundle {
    pub timeseries: Vec<TimeseriesRow>,
    pub rates: Vec<RateRow>,
}

const SERIES_SQL: &str = r#"
WITH cfg AS (
  SELECT
    $1::text AS exp_key,
    CASE WHEN $2::text LIKE '%\_rate'
         THEN left($2::text, length($2::text) - 5)
         ELSE $2::text END AS num_event
),
scoped AS (
  SELECT
    e.received_at,
    e.body->'properties'->>'variant' AS variant,
    e.body->>'event'                 AS event,
    COALESCE(e.body->'user'->>'id', e.body->'user'->>'username',
             e.body->>'userId', e.body->>'anonymousId') AS user_key
  FROM telemetry.events e, cfg
  WHERE e.source = 'segment'
    AND e.body->'properties'->>'exp_key' = cfg.exp_key
    AND e.body->'properties'->>'variant' IS NOT NULL
)
SELECT
  (SELECT COALESCE(jsonb_agg(jsonb_build_object(
      'day', t.day, 'variant', t.variant, 'exposures', t.exposures, 'conversions', t.conversions)
      ORDER BY t.day, t.variant), '[]'::jsonb)
   FROM (
     SELECT
       to_char(s.received_at, 'YYYY-MM-DD') AS day,
       s.variant,
       count(DISTINCT s.user_key) FILTER (WHERE s.event = 'experiment_exposed') AS exposures,
       count(DISTINCT s.user_key) FILTER (WHERE s.event = (SELECT num_event FROM cfg)) AS conversions
     FROM scoped s
     GROUP BY 1, 2
   ) t) AS timeseries,
  (SELECT COALESCE(jsonb_agg(jsonb_build_object(
      'variant', r.variant, 'exposures', r.exposures, 'successes', r.successes,
      'rate', CASE WHEN r.exposures > 0 THEN r.successes::double precision / r.exposures ELSE 0 END)
      ORDER BY r.variant), '[]'::jsonb)
   FROM (
     SELECT
       s.variant,
       count(DISTINCT s.user_key) FILTER (WHERE s.event = 'experiment_exposed') AS exposures,
       count(DISTINCT s.user_key) FILTER (WHERE s.event = (SELECT num_event FROM cfg)) AS successes
     FROM scoped s
     GROUP BY s.variant
   ) r) AS rates
"#;

async fn series(st: &SharedState, p: &SeriesQuery) -> Result<SeriesBundle, (StatusCode, String)> {
    let key = (p.exp_key.clone(), p.metric.clone());
    if is_fresh(p.fresh.as_deref()) {
        st.series_cache.clear();
    }
    let pool = &st.pool;
    st.series_cache
        .get_or_fetch(key, || async {
            let (timeseries, rates): (Value, Value) = sqlx::query_as(SERIES_SQL)
                .bind(&p.exp_key)
                .bind(&p.metric)
                .fetch_one(pool)
                .await?;
            Ok(SeriesBundle {
                timeseries: serde_json::from_value(timeseries).map_err(sqlx::Error::decode)?,
                rates: serde_json::from_value(rates).map_err(sqlx::Error::decode)?,
            })
        })
        .await
        .map_err(|e| db_err("telemetry experiments", e))
}

pub async fn timeseries(
    State(st): State<SharedState>,
    Query(p): Query<SeriesQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = series(&st, &p).await?.timeseries;
    Ok(Json(
        json!({ "exp_key": p.exp_key, "metric": p.metric, "rows": rows }),
    ))
}

pub async fn rates(
    State(st): State<SharedState>,
    Query(p): Query<SeriesQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = series(&st, &p).await?.rates;
    Ok(Json(
        json!({ "exp_key": p.exp_key, "metric": p.metric, "rows": rows }),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

    use super::super::Memo;
    use super::*;

    fn row(key: &str, variant: Option<&str>, event: Option<&str>, n: i64) -> ListRow {
        (
            key.to_string(),
            variant.map(str::to_string),
            event.map(str::to_string),
            n,
        )
    }

    fn counting(
        loads: &AtomicUsize,
    ) -> impl Fn() -> std::future::Ready<Result<Value, String>> + '_ {
        move || std::future::ready(Ok(json!(loads.fetch_add(1, SeqCst) + 1)))
    }

    #[test]
    fn aggregate_keeps_the_response_shape() {
        let out = aggregate(vec![
            row("hud_cta", Some("control"), Some("experiment_exposed"), 40),
            row("hud_cta", Some("blue"), Some("experiment_exposed"), 42),
            row("hud_cta", Some("blue"), Some("cta_clicked"), 7),
            row("hud_cta", Some("control"), Some("cta_clicked"), 5),
            row("hud_cta", None, None, 1),
            row("no_control", Some("b"), Some("experiment_exposed"), 3),
            row("no_control", Some("a"), Some("experiment_exposed"), 3),
            row(
                "readout_probe_x",
                Some("control"),
                Some("experiment_exposed"),
                9,
            ),
            row("single", Some("control"), Some("experiment_exposed"), 9),
            row("unexposed", Some("a"), Some("clicked"), 9),
            row("unexposed", Some("b"), Some("clicked"), 9),
        ]);
        assert_eq!(
            out,
            json!({
                "experiments": [
                    {"exp_key": "hud_cta", "exposures": 82, "variants": ["blue", "control"],
                     "metrics": [{"event": "cta_clicked", "count": 12}], "control": "control"},
                    {"exp_key": "no_control", "exposures": 6, "variants": ["a", "b"],
                     "metrics": [], "control": "a"},
                ],
                "unreadable": [
                    {"exp_key": "readout_probe_x", "exposures": 9, "variants": ["control"],
                     "metrics": [], "reason": "synthetic key (matches exclusion pattern)"},
                    {"exp_key": "single", "exposures": 9, "variants": ["control"],
                     "metrics": [], "reason": "fewer than 2 variants"},
                    {"exp_key": "unexposed", "exposures": 0, "variants": ["a", "b"],
                     "metrics": [{"event": "clicked", "count": 18}],
                     "reason": "no experiment_exposed events"},
                ]
            })
        );
    }

    #[test]
    fn list_sql_matches_the_partial_index_definition() {
        let schema = crate::db::SCHEMA_SQL;
        let start = schema
            .find("CREATE INDEX events_experiments_idx")
            .expect("the partial index");
        let index = &schema[start..start + 400];
        for needle in [
            "source = 'segment'",
            "body->'properties'->>'exp_key' IS NOT NULL",
            "body->'properties'->>'exp_key'",
            "body->'properties'->>'variant'",
            "body->>'event'",
        ] {
            assert!(LIST_SQL.contains(needle), "query lacks {needle}");
            assert!(index.contains(needle), "index lacks {needle}");
        }
    }

    #[test]
    fn fresh_param_zeroes_the_ttl() {
        assert!(!is_fresh(None));
        assert!(!is_fresh(Some("0")));
        assert!(is_fresh(Some("1")));
        assert!(is_fresh(Some("true")));
    }

    #[tokio::test(start_paused = true)]
    async fn polls_within_the_ttl_share_one_aggregate() {
        let cell = Memo::<Value>::new(LIST_TTL);
        let loads = AtomicUsize::new(0);
        let load = counting(&loads);

        let a = cell.get_or_refresh(false, &load).await.unwrap();
        let b = cell.get_or_refresh(false, &load).await.unwrap();
        assert_eq!((a, b), (json!(1), json!(1)));

        tokio::time::advance(LIST_TTL - Duration::from_secs(1)).await;
        assert_eq!(cell.get_or_refresh(false, &load).await.unwrap(), json!(1));
        tokio::time::advance(Duration::from_secs(1)).await;
        assert_eq!(cell.get_or_refresh(false, &load).await.unwrap(), json!(2));
        assert_eq!(loads.load(SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn fresh_reloads_and_refreshes_the_memo() {
        let cell = Memo::<Value>::new(LIST_TTL);
        let loads = AtomicUsize::new(0);
        let load = counting(&loads);

        assert_eq!(cell.get_or_refresh(false, &load).await.unwrap(), json!(1));
        assert_eq!(cell.get_or_refresh(true, &load).await.unwrap(), json!(2));
        assert_eq!(cell.get_or_refresh(false, &load).await.unwrap(), json!(2));
        assert_eq!(loads.load(SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_polls_do_not_stampede() {
        let cell = Memo::<Value>::new(LIST_TTL);
        let loads = AtomicUsize::new(0);
        let release = tokio::sync::Notify::new();
        let (loads, release) = (&loads, &release);
        let mut polls = std::pin::pin!(futures::future::join_all((0..8).map(|_| {
            cell.get_or_refresh(false, move || async move {
                release.notified().await;
                Ok::<_, String>(json!(loads.fetch_add(1, SeqCst) + 1))
            })
        })));
        // One poll queues every caller behind the first one's load, which the test holds.
        assert!(futures::FutureExt::now_or_never(polls.as_mut()).is_none());
        release.notify_one();
        for value in polls.await {
            assert_eq!(value.unwrap(), json!(1));
        }
        assert_eq!(loads.load(SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_refresh_serves_the_last_good_value() {
        let cell = Memo::<Value>::new(Duration::ZERO);
        assert_eq!(
            cell.get_or_stale(|| async { Ok::<_, String>(json!(1)) })
                .await,
            Some(json!(1))
        );
        assert_eq!(
            cell.get_or_stale(|| async { Err::<Value, _>("down".to_string()) })
                .await,
            Some(json!(1))
        );
        let empty = Memo::<Value>::new(Duration::ZERO);
        assert_eq!(
            empty
                .get_or_stale(|| async { Err::<Value, _>("down".to_string()) })
                .await,
            None
        );
    }

    /// The numbers the sites e2e (`experiment-readout.e2e.test.ts`) pins
    /// against `ext.two_prop_z/p`: 50/100 vs 60/100.
    #[test]
    fn the_statistics_match_the_sql_helpers() {
        let z = stats::two_prop_z(50.0, 100.0, 60.0, 100.0);
        assert!((z - 1.4213381).abs() < 1e-6, "{z}");
        let p = stats::two_prop_p(50.0, 100.0, 60.0, 100.0);
        assert!((p - 0.155218).abs() < 1e-5, "{p}");
        assert_eq!(stats::two_prop_z(0.0, 0.0, 5.0, 10.0), 0.0);
        assert_eq!(stats::two_prop_p(0.0, 0.0, 5.0, 10.0), 1.0);
        assert_eq!(
            stats::two_prop_p(5.0, 10.0, 5.0, 10.0),
            1.0,
            "equal rates carry no signal"
        );
        assert!((stats::norm_cdf(0.0) - 0.5).abs() < 1e-7);
        assert!((stats::norm_cdf(1.96) - 0.9750021).abs() < 1e-6);
        assert!((stats::beta_post_mean(3.0, 10.0) - 4.0 / 12.0).abs() < 1e-12);
        assert!((stats::beta_post_var(3.0, 10.0) - (4.0 * 8.0) / (144.0 * 13.0)).abs() < 1e-12);
        let pb = stats::p_beats(50.0, 100.0, 60.0, 100.0);
        assert!(pb > 0.9 && pb < 1.0, "{pb}");
    }

    #[test]
    fn readout_rows_project_every_variant_against_the_control() {
        let arms: Vec<Arm> = vec![
            ("blue".into(), 1000, 150),
            ("control".into(), 1000, 100),
            ("red".into(), 1000, 60),
        ];
        let rows = readout_rows(&arms, "control", None, None);
        assert_eq!(
            rows.iter().map(|r| r.variant.as_str()).collect::<Vec<_>>(),
            vec!["blue", "red"]
        );
        let blue = &rows[0];
        assert_eq!((blue.n_exposures, blue.successes), (1000, 150));
        assert!((blue.rate - 0.15).abs() < 1e-12 && (blue.control_rate - 0.10).abs() < 1e-12);
        assert!(blue.significant && blue.verdict == "SHIP", "{blue:?}");
        assert!(blue.p_beats_control > 0.99);
        assert!(blue.bayes_ci_low < blue.bayes_mean && blue.bayes_mean < blue.bayes_ci_high);
        let red = &rows[1];
        assert!(red.significant && red.verdict == "KILL", "{red:?}");
        let held = readout_rows(&arms, "control", None, Some(5000.0));
        assert_eq!(
            held[0].verdict, "KEEP RUNNING",
            "min_sample holds a SHIP back"
        );
        assert!(
            readout_rows(&arms, "missing", None, None).is_empty(),
            "no control arm, no rows"
        );
        let empty = readout_rows(
            &[("control".into(), 0, 0), ("x".into(), 0, 0)],
            "control",
            None,
            None,
        );
        assert_eq!(empty[0].verdict, "KEEP RUNNING");
        assert_eq!(
            (
                empty[0].rate,
                empty[0].control_rate,
                empty[0].z,
                empty[0].p_value
            ),
            (0.0, 0.0, 0.0, 1.0)
        );
    }
}
