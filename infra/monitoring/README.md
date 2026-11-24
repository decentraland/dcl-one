# Catalyst index alerts

Every `catalyst index` process starts its lag monitor and binds
`127.0.0.1:5008` by default. Before deploying a second index process in the
same network namespace, set `INDEX_METRICS_BIND` to an available loopback
address for that instance. An occupied socket prevents startup. Scrape its `/metrics` endpoint every minute with
the Prometheus job name `catalyst-index`. Use a collector in the same
network namespace or a private scrape proxy. Keep each expected index
instance in target discovery across process restarts. When a target can
disappear with its collector, supply one independent recording rule on
the evaluating Prometheus per expected instance, with the scrape target's
`job`, `instance` and, when present, `cluster` labels.

```yaml
- record: catalyst_index_target_expected
  expr: vector(1)
  labels: {job: catalyst-index, instance: "<expected instance>"}
```

This deployment-owned inventory lets the missing-metrics rule identify a
removed instance while its peers still report.

Load `index.rules.yml` through Prometheus `rule_files`. Configure its
`alerting.alertmanagers` and add this child route under the deployment's
Alertmanager `route.routes`. Bind the receiver to the operators' channel
using the deployment's existing receiver credentials.

```yaml
- matchers: ['job="catalyst-index"', 'team="operators"']
  receiver: operators
```

The index worker checks the configured shop projections, LAND cursor and
mirrors every minute. Projections in the same process that use one RPC
endpoint share a head probe. The lag gauge measures the chain head minus the persisted last block.
The threshold includes the index's confirmation depth and publication
allowance. Rules use the most recent valid lag from the last four minutes, so a single
missed head probe preserves lag, stall and unpublished warning timers. A walk held at its configured stop block, or in quarantine, is
excluded from both. A stationary block alerts while an active walk exceeds
its lag threshold. A quarantine alerts even if the RPC is down. The published
fold has its own height and age. Readers require a publication within
30 minutes. A publication two hours old triggers the stale-publication
warning. A full walk that stays near the head for
30 minutes without a publication also alerts. The persisted full-history
or replay flag determines whether a cursor can publish. Mirror age
measures seconds since the last complete pass, using the freshness budget
of the mirror's readers.

An unavailable height or age is `NaN`. A configured source with no row or
usable head, including an enabled index missing its RPC, sets
`catalyst_index_source_observed` to zero. LAND checks the configured
registry pair as well as the network. A failed database read leaves the
sampler's timestamp at the last successful read. Disabled
jobs have no expected series. The deployment owns target discovery, rule
loading and receiver configuration.

Run both checks from this directory.

```sh
promtool check rules index.rules.yml
promtool test rules index.test.yml
```
