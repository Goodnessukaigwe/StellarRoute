# Design Document: Liquidity Health Scoring

## Overview

This feature adds a health scoring layer between the indexer's raw liquidity data and the routing
pathfinder. Each venue (SDEX orderbook offer or AMM pool) receives a numeric health score in
[0.0, 1.0] computed from observable signals. A configurable exclusion policy removes degraded venues
from the routing graph before pathfinding begins. Operators can override the policy per venue via
config. Excluded venues are reported in the `QuoteResponse` as `exclusion_diagnostics`.

The feature spans three crates:

- `routing` — `HealthScorer`, `ExclusionPolicy`, `OverrideRegistry`, graph filtering
- `api` — `exclusion_diagnostics` field on `QuoteResponse`, wiring into `AppState`
- `indexer` — migration for `venue_health_scores` table, persistence writer

---

## Architecture

```mermaid
flowchart TD
    subgraph indexer["indexer crate"]
        DB[(PostgreSQL)]
        Writer["HealthScoreWriter\n(persists to venue_health_scores)"]
    end

    subgraph routing["routing crate"]
        Scorer["HealthScorer\n(SdexScorer / AmmScorer)"]
        Policy["ExclusionPolicy\n(thresholds + OverrideRegistry)"]
        Filter["GraphFilter\n(removes degraded edges)"]
        Pathfinder["Pathfinder (existing)"]
    end

    subgraph api["api crate"]
        QuoteRoute["GET /quote handler"]
        Response["QuoteResponse\n+ exclusion_diagnostics"]
    end

    DB -->|normalized_liquidity rows| Scorer
    Scorer -->|HealthRecord| Writer
    Writer -->|INSERT| DB
    Scorer -->|Vec<ScoredVenue>| Policy
    Policy -->|excluded set + diagnostics| Filter
    Filter -->|filtered LiquidityEdge[]| Pathfinder
    Pathfinder -->|SwapPath| QuoteRoute
    Policy -->|ExclusionDiagnostics| QuoteRoute
    QuoteRoute --> Response
```

The `HealthScorer` is called once per quote request, operating on the same venue rows already
fetched from `normalized_liquidity`. The `GraphFilter` wraps the existing `Pathfinder::find_paths`
call, so no changes are needed inside the BFS algorithm itself.

---

## Components and Interfaces

### HealthScorer (`routing/src/health/scorer.rs`)

```rust
pub trait VenueScorer: Send + Sync {
    fn score(&self, input: &VenueScorerInput) -> HealthRecord;
}

pub struct SdexScorer {
    pub staleness_threshold_secs: u64,
}

pub struct AmmScorer {
    pub staleness_threshold_secs: u64,
    pub min_tvl_threshold: i128,   // in e7 units
}

pub struct HealthScorer {
    pub sdex: SdexScorer,
    pub amm: AmmScorer,
}

impl HealthScorer {
    pub fn score_venues(&self, inputs: &[VenueScorerInput]) -> Vec<ScoredVenue>;
}
```

`VenueScorerInput` carries the raw signals read from `normalized_liquidity` plus the venue type.
`HealthRecord` carries the final score and the individual signal values.

### ExclusionPolicy (`routing/src/health/policy.rs`)

```rust
pub struct ExclusionThresholds {
    pub sdex: f64,   // default 0.5
    pub amm: f64,    // default 0.5
}

pub enum OverrideDirective {
    ForceInclude,
    ForceExclude,
}

pub struct OverrideRegistry {
    pub entries: HashMap<String, OverrideDirective>,  // keyed by venue_ref
}

pub struct ExclusionPolicy {
    pub thresholds: ExclusionThresholds,
    pub overrides: OverrideRegistry,
}

impl ExclusionPolicy {
    /// Returns (included_refs, ExclusionDiagnostics)
    pub fn apply(
        &self,
        scored: &[ScoredVenue],
    ) -> (HashSet<String>, ExclusionDiagnostics);
}
```

### GraphFilter (`routing/src/health/filter.rs`)

```rust
pub struct GraphFilter<'a> {
    policy: &'a ExclusionPolicy,
}

impl<'a> GraphFilter<'a> {
    /// Filters edges and returns (filtered_edges, diagnostics)
    pub fn filter_edges(
        &self,
        edges: &[LiquidityEdge],
        scored: &[ScoredVenue],
    ) -> (Vec<LiquidityEdge>, ExclusionDiagnostics);
}
```

### HealthScoreWriter (`indexer/src/db/health_scores.rs`)

```rust
pub struct HealthScoreWriter {
    pool: PgPool,
}

impl HealthScoreWriter {
    pub async fn write(&self, record: &HealthRecord) -> Result<()>;
}
```

Errors from `write` are logged and swallowed; they never propagate to the routing path.

### API integration (`api/src/routes/quote.rs`)

The `find_best_price` function is extended to:
1. Fetch venue rows from `normalized_liquidity`
2. Score them via `HealthScorer`
3. Apply `ExclusionPolicy` to get `ExclusionDiagnostics`
4. Pass filtered edges to the pathfinder
5. Attach `exclusion_diagnostics` to `QuoteResponse`

`AppState` gains a `health_config: HealthScoringConfig` field loaded from the application config.

---

## Data Models

### VenueScorerInput

```rust
pub struct VenueScorerInput {
    pub venue_ref: String,
    pub venue_type: VenueType,          // Sdex | Amm
    // SDEX signals
    pub best_bid_e7: Option<i128>,
    pub best_ask_e7: Option<i128>,
    pub depth_top_n_e7: Option<i128>,   // sum of top-N level amounts
    // AMM signals
    pub reserve_a_e7: Option<i128>,
    pub reserve_b_e7: Option<i128>,
    pub tvl_e7: Option<i128>,
    // Shared
    pub last_updated_at: chrono::DateTime<chrono::Utc>,
}
```

### HealthRecord

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthRecord {
    pub venue_ref: String,
    pub venue_type: VenueType,
    pub score: f64,                     // [0.0, 1.0]
    pub signals: serde_json::Value,     // JSONB-compatible signal map
    pub computed_at: chrono::DateTime<chrono::Utc>,
}
```

`signals` is a `serde_json::Value::Object` so the schema never needs to change when new signals
are added. Example for SDEX:

```json
{
  "spread_ratio": 0.0023,
  "depth_top5_e7": 5000000000,
  "staleness_secs": 4
}
```

### ScoredVenue

```rust
pub struct ScoredVenue {
    pub venue_ref: String,
    pub venue_type: VenueType,
    pub record: HealthRecord,
}
```

### ExclusionDiagnostics (API response type)

```rust
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExclusionDiagnostics {
    pub excluded_venues: Vec<ExcludedVenueInfo>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExcludedVenueInfo {
    pub venue_ref: String,
    pub score: f64,
    pub signals: serde_json::Value,
    pub reason: ExclusionReason,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    PolicyThreshold { threshold: f64 },
    Override,
}
```

### HealthScoringConfig

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct HealthScoringConfig {
    #[serde(default)]
    pub thresholds: ExclusionThresholds,
    #[serde(default)]
    pub overrides: Vec<OverrideEntry>,
    #[serde(default = "default_staleness_secs")]
    pub staleness_threshold_secs: u64,
    #[serde(default = "default_min_tvl")]
    pub min_tvl_threshold_e7: i128,
    #[serde(default = "default_depth_levels")]
    pub depth_levels: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OverrideEntry {
    pub venue_ref: String,
    pub directive: OverrideDirective,
}
```

Defaults: `sdex_threshold = 0.5`, `amm_threshold = 0.5`, `staleness_threshold_secs = 60`,
`min_tvl_threshold_e7 = 1_000_000_000` (100 units), `depth_levels = 5`.

### SDEX Score Formula

```
spread_ratio  = (best_ask - best_bid) / mid_price        ∈ [0, ∞)
spread_score  = clamp(1.0 - spread_ratio / MAX_SPREAD, 0.0, 1.0)
depth_score   = clamp(depth_top_n / TARGET_DEPTH, 0.0, 1.0)
staleness_score = if staleness_secs > threshold { 0.0 } else { 1.0 - staleness_secs / threshold }

score = 0.4 * spread_score + 0.4 * depth_score + 0.2 * staleness_score
```

Zero-bid or zero-ask → score = 0.0. Staleness beyond threshold → score = 0.0.

### AMM Score Formula

```
reserve_ratio_dev = |reserve_a * reserve_b - k_expected| / k_expected   (0 if first observation)
invariant_score   = clamp(1.0 - reserve_ratio_dev, 0.0, 1.0)
tvl_score         = clamp(tvl / min_tvl_threshold, 0.0, 1.0)
staleness_score   = if staleness_secs > threshold { 0.0 } else { 1.0 - staleness_secs / threshold }

score = 0.4 * invariant_score + 0.4 * tvl_score + 0.2 * staleness_score
```

Either reserve = 0 → score = 0.0. Staleness beyond threshold → score = 0.0.

### Database Migration (`0005_venue_health_scores.sql`)

```sql
create table if not exists venue_health_scores (
    id            bigserial primary key,
    venue_ref     text        not null,
    venue_type    text        not null check (venue_type in ('sdex', 'amm')),
    score         numeric(5, 4) not null check (score >= 0 and score <= 1),
    signals       jsonb       not null default '{}',
    computed_at   timestamptz not null default now()
);

create index if not exists idx_venue_health_scores_ref_time
    on venue_health_scores (venue_ref, computed_at desc);
```

### QuoteResponse extension

`QuoteResponse` gains one new optional field:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub exclusion_diagnostics: Option<ExclusionDiagnostics>,
```

---

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a
system — essentially, a formal statement about what the system should do. Properties serve as the
bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Health score is always in [0.0, 1.0]

*For any* `VenueScorerInput` (SDEX or AMM, with any combination of signal values), the
`HealthScorer` SHALL produce a `HealthRecord` whose `score` field satisfies `0.0 ≤ score ≤ 1.0`.

**Validates: Requirements 1.4, 2.4**

---

### Property 2: Zero-reserve or empty-book forces score to 0.0

*For any* `VenueScorerInput` where either reserve is zero (AMM) or bids/asks are absent (SDEX),
the resulting `HealthRecord.score` SHALL equal `0.0`.

**Validates: Requirements 1.2, 2.2**

---

### Property 3: Stale venue forces score to 0.0

*For any* `VenueScorerInput` whose `last_updated_at` is older than the configured
`staleness_threshold_secs`, the resulting `HealthRecord.score` SHALL equal `0.0`.

**Validates: Requirements 1.3, 2.3**

---

### Property 4: Exclusion policy respects threshold

*For any* set of scored venues and any threshold `t ∈ [0.0, 1.0]`, every venue whose score is
strictly less than `t` and is not force-included SHALL appear in the excluded set returned by
`ExclusionPolicy::apply`.

**Validates: Requirements 3.1, 3.3**

---

### Property 5: force_include overrides low score

*For any* venue listed in the `OverrideRegistry` with `force_include`, that venue SHALL NOT appear
in the excluded set regardless of its health score.

**Validates: Requirements 4.2**

---

### Property 6: force_exclude overrides high score

*For any* venue listed in the `OverrideRegistry` with `force_exclude`, that venue SHALL appear in
the excluded set with `reason = ExclusionReason::Override`, regardless of its health score.

**Validates: Requirements 4.3, 5.4**

---

### Property 7: Filtered graph contains no excluded edges

*For any* set of `LiquidityEdge` values and any `ExclusionPolicy`, every edge whose `venue_ref`
appears in the excluded set SHALL be absent from the filtered edge list returned by
`GraphFilter::filter_edges`.

**Validates: Requirements 3.3**

---

### Property 8: Health score serialization round-trip

*For any* valid `HealthRecord`, serializing to JSON and then deserializing SHALL produce a record
equal to the original.

**Validates: Requirements 7.2**

---

### Property 9: Unknown JSON fields are ignored on deserialization

*For any* valid `HealthRecord` JSON payload with additional unknown fields appended, deserialization
SHALL succeed and the known fields SHALL equal those of the original record.

**Validates: Requirements 7.3**

---

## Error Handling

| Scenario | Behavior |
|---|---|
| DB unavailable during `HealthScoreWriter::write` | Log error at `warn` level; return `Ok(())` to caller; routing continues |
| Unrecognized `venue_ref` in `OverrideRegistry` | Log `warn` with the identifier; continue routing without error |
| Missing required field in `HealthRecord` JSON | Return descriptive `serde` parse error identifying the field name |
| Score computation overflow (e.g. extreme reserve values) | Clamp to 0.0; log at `debug` level |
| All venues excluded (empty filtered graph) | Pathfinder returns `RoutingError::NoRoute`; API returns 404 |

---

## Testing Strategy

### Unit tests

Focus on specific examples, edge cases, and error conditions:

- `SdexScorer`: zero-bid, zero-ask, stale timestamp, normal healthy input
- `AmmScorer`: zero reserve, stale timestamp, TVL below threshold, normal healthy input
- `ExclusionPolicy`: threshold boundary (score == threshold is NOT excluded), force_include,
  force_exclude, unrecognized override key (warning only)
- `GraphFilter`: all excluded, none excluded, partial exclusion
- `HealthScoreWriter`: DB error is swallowed and logged
- `QuoteResponse` serialization: `exclusion_diagnostics` omitted when `None`, present when `Some`

### Property-based tests

Use the [`proptest`](https://crates.io/crates/proptest) crate (already compatible with the
workspace's Rust edition). Each property test runs a minimum of 100 iterations.

Each test is tagged with a comment in the format:
`// Feature: liquidity-health-scoring, Property N: <property_text>`

| Property | Test description |
|---|---|
| P1 | Generate arbitrary `VenueScorerInput`; assert `0.0 ≤ score ≤ 1.0` |
| P2 | Generate inputs with zero reserve / empty book; assert `score == 0.0` |
| P3 | Generate inputs with `last_updated_at` older than threshold; assert `score == 0.0` |
| P4 | Generate scored venue lists and thresholds; assert all sub-threshold venues are excluded |
| P5 | Generate scored venues with force_include entries; assert none appear in excluded set |
| P6 | Generate scored venues with force_exclude entries; assert all appear with `Override` reason |
| P7 | Generate edge lists and exclusion sets; assert no excluded edge survives filtering |
| P8 | Generate arbitrary `HealthRecord`; assert `deserialize(serialize(r)) == r` |
| P9 | Generate `HealthRecord` JSON + random extra fields; assert deserialization succeeds |

**Property test configuration:**

```toml
# crates/routing/Cargo.toml [dev-dependencies]
proptest = "1"
```

Each property test uses `proptest!` macro with `#[cfg(test)]` and references its design property
in a leading comment.
