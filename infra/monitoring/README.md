# Catalyst index alerts

Set `INDEX_METRICS_BIND` on the `catalyst index` process to an allocated
loopback socket address. Scrape its `/metrics` endpoint every minute with
the Prometheus job name `catalyst-index`. Use a collector in the same
network namespace or a private scrape proxy. Keep each expected index
instance in target discovery across process restarts.

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
allowance. A stationary block only alerts while the index exceeds that
threshold. A quarantine alerts even if the RPC is down. The published
fold has its own height and age. A full walk that stays near the head for
30 minutes without a publication also alerts. Partial and stop-bounded
development walks are excluded from that publication warning. Mirror age
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
