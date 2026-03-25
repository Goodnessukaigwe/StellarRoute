# Implementation Plan: Liquidity Health Scoring

## Overview

Implement the health scoring layer across three crates (`routing`, `api`, `indexer`). Tasks proceed
from core data types → scoring logic → exclusion policy → graph filtering → persistence → API
wiring, ensuring each step integrates cleanly before the next begins.

## Tasks

- [x] 1. Define shared data types and module skeleton
  - Create `routing/src/health/mod.rs` exporting `scorer`, `policy`, `filter` submodules
  - Define `VenueType`, `VenueScorerInput`, `HealthRecord`, `ScoredVenue` in `routing/src/health/scorer.rs`
  - Define `ExclusionThresholds`, `OverrideDirective`, `OverrideEntry`, `OverrideRegistry`, `ExclusionPolicy` structs in `routing/src/health/policy.rs`
  - Define `ExclusionDiagnostics`, `ExcludedVenueInfo`, `ExclusionReason` in `routing/src/health/policy.rs`
  - Define `HealthScoringConfig` with `serde::Deserialize` and all defaults in `routing/src/health/scorer.rs`
  - Add `pub mod health;` to `routing/src/lib.rs`
  - _Requirements: 1.4, 1.5, 2.4, 2.5, 3.1, 3.2, 3.5, 5.2, 7.1_

- [ ] 2. Implement SDEX and AMM scorers
  - [x] 2.1 Implement `SdexScorer::score` using the weighted formula (0.4·spread + 0.4·depth + 0.2·staleness)
    - Return score 0.0 immediately when bids or asks are absent
    - Return score 0.0 when `last_updated_at` is older than `staleness_threshold_secs`
    - Clamp all intermediate and final values to [0.0, 1.0]
    - Populate `signals` JSON object with `spread_ratio`, `depth_top_n_e7`, `staleness_secs`
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

  - [ ]* 2.2 Write property test for Property 1 (score always in [0.0, 1.0])
    - `// Feature: liquidity-health-scoring, Property 1: Health score is always in [0.0, 1.0]`
    - Generate arbitrary `VenueScorerInput` for both SDEX and AMM; assert `0.0 <= score <= 1.0`
    - **Property 1: Health score is always in [0.0, 1.0]**
    - **Validates: Requirements 1.4, 2.4**

  - [ ]* 2.3 Write property test for Property 2 (zero-reserve / empty-book forces 0.0)
    - `// Feature: liquidity-health-scoring, Property 2: Zero-reserve or empty-book forces score to 0.0`
    - Generate SDEX inputs with `best_bid_e7 = None` or `best_ask_e7 = None`; generate AMM inputs with `reserve_a_e7 = 0` or `reserve_b_e7 = 0`; assert `score == 0.0`
    - **Property 2: Zero-reserve or empty-book forces score to 0.0**
    - **Validates: Requirements 1.2, 2.2**

  - [x] 2.4 Implement `AmmScorer::score` using the weighted formula (0.4·invariant + 0.4·tvl + 0.2·staleness)
    - Return score 0.0 immediately when either reserve is zero
    - Return score 0.0 when `last_updated_at` is older than `staleness_threshold_secs`
    - Clamp all intermediate and final values to [0.0, 1.0]; log at `debug` on overflow
    - Populate `signals` JSON object with `reserve_ratio_dev`, `tvl_e7`, `staleness_secs`
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

  - [ ]* 2.5 Write property test for Property 3 (stale venue forces 0.0)
    - `// Feature: liquidity-health-scoring, Property 3: Stale venue forces score to 0.0`
    - Generate inputs where `last_updated_at` is older than `staleness_threshold_secs`; assert `score == 0.0`
    - **Property 3: Stale venue forces score to 0.0**
    - **Validates: Requirements 1.3, 2.3**

  - [x] 2.6 Implement `HealthScorer::score_venues` to dispatch to `SdexScorer` or `AmmScorer` per venue type
    - _Requirements: 1.1, 2.1_

- [x] 3. Checkpoint — ensure scorer unit tests pass
  - Add unit tests for `SdexScorer`: zero-bid, zero-ask, stale timestamp, healthy input
  - Add unit tests for `AmmScorer`: zero reserve, stale timestamp, TVL below threshold, healthy input
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Implement ExclusionPolicy and OverrideRegistry
  - [x] 4.1 Implement `ExclusionPolicy::apply` returning `(HashSet<String>, ExclusionDiagnostics)`
    - Venues with `force_exclude` override → add to excluded set with `ExclusionReason::Override`
    - Venues with `force_include` override → skip threshold check entirely
    - Venues with `score < threshold` (per venue type) and no override → add with `ExclusionReason::PolicyThreshold { threshold }`
    - Log `warn` for any `venue_ref` in `OverrideRegistry` not present in the scored list
    - _Requirements: 3.1, 3.2, 4.1, 4.2, 4.3, 4.5, 5.2, 5.4_

  - [ ]* 4.2 Write property test for Property 4 (exclusion policy respects threshold)
    - `// Feature: liquidity-health-scoring, Property 4: Exclusion policy respects threshold`
    - Generate arbitrary scored venue lists and thresholds; assert every sub-threshold non-force-included venue is excluded
    - **Property 4: Exclusion policy respects threshold**
    - **Validates: Requirements 3.1, 3.3**

  - [ ]* 4.3 Write property test for Property 5 (force_include overrides low score)
    - `// Feature: liquidity-health-scoring, Property 5: force_include overrides low score`
    - Generate scored venues with force_include entries; assert none appear in excluded set
    - **Property 5: force_include overrides low score**
    - **Validates: Requirements 4.2**

  - [ ]* 4.4 Write property test for Property 6 (force_exclude overrides high score)
    - `// Feature: liquidity-health-scoring, Property 6: force_exclude overrides high score`
    - Generate scored venues with force_exclude entries; assert all appear with `ExclusionReason::Override`
    - **Property 6: force_exclude overrides high score**
    - **Validates: Requirements 4.3, 5.4**

- [ ] 5. Implement GraphFilter
  - [x] 5.1 Implement `GraphFilter::filter_edges` to remove any `LiquidityEdge` whose `venue_ref` is in the excluded set
    - Return `(Vec<LiquidityEdge>, ExclusionDiagnostics)` where diagnostics come from `ExclusionPolicy::apply`
    - _Requirements: 3.3_

  - [ ]* 5.2 Write property test for Property 7 (filtered graph contains no excluded edges)
    - `// Feature: liquidity-health-scoring, Property 7: Filtered graph contains no excluded edges`
    - Generate arbitrary edge lists and exclusion sets; assert no excluded edge survives filtering
    - **Property 7: Filtered graph contains no excluded edges**
    - **Validates: Requirements 3.3**

- [x] 6. Checkpoint — ensure policy and filter tests pass
  - Add unit tests for `ExclusionPolicy`: threshold boundary (score == threshold is NOT excluded), force_include, force_exclude, unrecognized override key
  - Add unit tests for `GraphFilter`: all excluded, none excluded, partial exclusion
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 7. Implement HealthRecord serialization and property tests
  - [x] 7.1 Derive `Serialize`, `Deserialize` on `HealthRecord` with `#[serde(rename_all = "snake_case")]`; add `#[serde(deny_unknown_fields)]` only on internal structs where appropriate; ensure `serde_json::Value` signals field round-trips correctly
    - _Requirements: 7.1, 7.2, 7.3, 7.4_

  - [ ]* 7.2 Write property test for Property 8 (round-trip serialization)
    - `// Feature: liquidity-health-scoring, Property 8: Health score serialization round-trip`
    - Generate arbitrary `HealthRecord` values; assert `deserialize(serialize(r)) == r`
    - **Property 8: Health score serialization round-trip**
    - **Validates: Requirements 7.2**

  - [ ]* 7.3 Write property test for Property 9 (unknown JSON fields ignored)
    - `// Feature: liquidity-health-scoring, Property 9: Unknown JSON fields are ignored on deserialization`
    - Generate valid `HealthRecord` JSON and append random extra fields; assert deserialization succeeds and known fields match
    - **Property 9: Unknown JSON fields are ignored on deserialization**
    - **Validates: Requirements 7.3**

- [ ] 8. Add database migration and HealthScoreWriter
  - [x] 8.1 Create `indexer/migrations/0005_venue_health_scores.sql` with the `venue_health_scores` table and `idx_venue_health_scores_ref_time` index
    - _Requirements: 6.1, 6.2, 6.3_

  - [x] 8.2 Implement `HealthScoreWriter::write` in `indexer/src/db/health_scores.rs`
    - INSERT `venue_ref`, `venue_type`, `score`, `signals` (JSONB), `computed_at` into `venue_health_scores`
    - On DB error: log at `warn` level and return `Ok(())` — never propagate to routing path
    - _Requirements: 6.1, 6.2, 6.4_

  - [ ]* 8.3 Write unit test for HealthScoreWriter DB error swallowing
    - Use a mock/offline pool to trigger a DB error; assert the function returns `Ok(())` and logs a warning
    - _Requirements: 6.4_

- [ ] 9. Wire health scoring into AppState and quote handler
  - [x] 9.1 Add `health_config: HealthScoringConfig` field to `AppState` in `api/src/state.rs`, loaded from the application config file
    - _Requirements: 3.4, 4.4_

  - [x] 9.2 Extend `QuoteResponse` in `api/src/models/response.rs` with `#[serde(skip_serializing_if = "Option::is_none")] pub exclusion_diagnostics: Option<ExclusionDiagnostics>`
    - _Requirements: 5.1, 5.5_

  - [x] 9.3 Update `find_best_price` (or equivalent) in `api/src/routes/quote.rs` to:
    - Build `Vec<VenueScorerInput>` from the fetched venue rows
    - Call `HealthScorer::score_venues`
    - Spawn async write to `HealthScoreWriter` (fire-and-forget, errors swallowed)
    - Call `GraphFilter::filter_edges` to get filtered edges and `ExclusionDiagnostics`
    - Pass filtered edges to the pathfinder
    - Attach `exclusion_diagnostics` to `QuoteResponse`
    - _Requirements: 3.3, 5.1, 5.2, 5.3, 6.1_

  - [ ]* 9.4 Write unit tests for QuoteResponse serialization
    - Assert `exclusion_diagnostics` is omitted from JSON when `None`
    - Assert `exclusion_diagnostics` is present and snake_case when `Some`
    - _Requirements: 5.3, 5.5_

- [x] 10. Final checkpoint — ensure all tests pass
  - Add `proptest = "1"` to `routing/Cargo.toml` `[dev-dependencies]` if not already present
  - Ensure all unit tests and property tests across `routing`, `api`, and `indexer` pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for a faster MVP
- Each task references specific requirements for traceability
- Property tests use the `proptest` crate with a minimum of 100 iterations per property
- `HealthScoreWriter` errors must never surface to the routing path (fire-and-forget pattern)
- Score boundary: `score < threshold` is excluded; `score == threshold` is NOT excluded (req 3.1)
