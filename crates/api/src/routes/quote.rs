//! Quote endpoint

use axum::{
    extract::{Path, Query, State},
    Json,
};
use sqlx::Row;
use std::sync::Arc;
use tracing::debug;

use stellarroute_routing::health::filter::GraphFilter;
use stellarroute_routing::health::policy::{ExclusionPolicy, OverrideRegistry};
use stellarroute_routing::health::scorer::{
    AmmScorer, HealthScorer, HealthScoringConfig, SdexScorer, VenueScorerInput, VenueType,
};

use crate::{
    cache,
    error::{ApiError, Result},
    models::{
        request::{AssetPath, QuoteParams},
        AssetInfo, ExcludedVenueInfo as ApiExcludedVenueInfo,
        ExclusionDiagnostics as ApiExclusionDiagnostics, ExclusionReason as ApiExclusionReason,
        PathStep, QuoteRationaleMetadata, QuoteResponse, VenueEvaluation,
    },
    state::AppState,
};

/// Get price quote for a trading pair
///
/// Returns the best available price for trading the specified amount
#[utoipa::path(
    get,
    path = "/api/v1/quote/{base}/{quote}",
    tag = "trading",
    params(
        ("base" = String, Path, description = "Base asset (e.g., 'native', 'USDC', or 'USDC:ISSUER')"),
        ("quote" = String, Path, description = "Quote asset (e.g., 'native', 'USDC', or 'USDC:ISSUER')"),
        ("amount" = Option<String>, Query, description = "Amount to trade (default: 1)"),
        ("slippage_bps" = Option<u32>, Query, description = "Slippage tolerance in basis points (default: 50)"),
        ("quote_type" = Option<String>, Query, description = "Type of quote: 'sell' or 'buy' (default: sell)"),
    ),
    responses(
        (status = 200, description = "Price quote", body = QuoteResponse),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 404, description = "No route found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    )
)]
pub async fn get_quote(
    State(state): State<Arc<AppState>>,
    Path((base, quote)): Path<(String, String)>,
    Query(params): Query<QuoteParams>,
) -> Result<Json<QuoteResponse>> {
    debug!(
        "Getting quote for {}/{} with params: {:?}",
        base, quote, params
    );

    // Parse asset identifiers
    let base_asset = AssetPath::parse(&base)
        .map_err(|e| ApiError::InvalidAsset(format!("Invalid base asset: {}", e)))?;
    let quote_asset = AssetPath::parse(&quote)
        .map_err(|e| ApiError::InvalidAsset(format!("Invalid quote asset: {}", e)))?;

    // Parse amount (default to 1)
    let amount: f64 = params
        .amount
        .as_deref()
        .unwrap_or("1")
        .parse()
        .map_err(|_| ApiError::Validation("Invalid amount".to_string()))?;

    if amount <= 0.0 {
        return Err(ApiError::Validation(
            "Amount must be greater than zero".to_string(),
        ));
    }

    let slippage_bps = params.slippage_bps.unwrap_or(50);
    if slippage_bps > 10_000 {
        return Err(ApiError::Validation(
            "slippage_bps must be between 0 and 10000".to_string(),
        ));
    }
    let quote_type = match params.quote_type {
        crate::models::request::QuoteType::Sell => "sell",
        crate::models::request::QuoteType::Buy => "buy",
    };

    let base_id = find_asset_id(&state, &base_asset).await?;
    let quote_id = find_asset_id(&state, &quote_asset).await?;

    maybe_invalidate_quote_cache(&state, &base, &quote, base_id, quote_id).await?;

    // Try to get from cache first
    let amount_str = format!("{:.7}", amount);
    let quote_cache_key = cache::keys::quote(&base, &quote, &amount_str, slippage_bps, quote_type);
    if let Some(cache) = &state.cache {
        if let Ok(mut cache) = cache.try_lock() {
            if let Some(cached) = cache.get::<QuoteResponse>(&quote_cache_key).await {
                state.cache_metrics.inc_quote_hit();
                debug!("Returning cached quote for {}/{}", base, quote);
                return Ok(Json(cached));
            }

            state.cache_metrics.inc_quote_miss();
        }
    }

    // For now, implement simple direct path (SDEX only)
    // TODO: Implement multi-hop routing in Phase 2
    let (price, path, rationale, api_diagnostics) =
        find_best_price(&state, &base_asset, &quote_asset, base_id, quote_id, amount).await?;

    let total = amount * price;
    // Keep timestamps in milliseconds to match API docs and frontend staleness logic.
    let timestamp = chrono::Utc::now().timestamp_millis();
    let ttl_seconds = u32::try_from(state.cache_policy.quote_ttl.as_secs()).ok();
    let expires_at = i64::try_from(state.cache_policy.quote_ttl.as_millis())
        .ok()
        .map(|ttl_ms| timestamp + ttl_ms);

    let response = QuoteResponse {
        base_asset: asset_path_to_info(&base_asset),
        quote_asset: asset_path_to_info(&quote_asset),
        amount: format!("{:.7}", amount),
        price: format!("{:.7}", price),
        total: format!("{:.7}", total),
        quote_type: quote_type.to_string(),
        path,
        timestamp,
        expires_at,
        source_timestamp: None,
        ttl_seconds,
        rationale: Some(rationale),
        exclusion_diagnostics: Some(api_diagnostics),
    };

    // Cache the response (TTL: 2 seconds for quote data)
    if let Some(cache) = &state.cache {
        if let Ok(mut cache) = cache.try_lock() {
            let _ = cache
                .set(&quote_cache_key, &response, state.cache_policy.quote_ttl)
                .await;
        }
    }

    Ok(Json(response))
}

/// Find best price for a trading pair
async fn find_best_price(
    state: &AppState,
    base: &AssetPath,
    quote: &AssetPath,
    base_id: uuid::Uuid,
    quote_id: uuid::Uuid,
    amount: f64,
) -> Result<(
    f64,
    Vec<PathStep>,
    QuoteRationaleMetadata,
    ApiExclusionDiagnostics,
)> {
    let rows = sqlx::query(
        r#"
                select
                    venue_type,
                    venue_ref,
                    price::text as price,
                    available_amount::text as available_amount
                from normalized_liquidity
        where selling_asset_id = $1
          and buying_asset_id = $2
        order by price asc, venue_type asc, venue_ref asc
        "#,
    )
    .bind(base_id)
    .bind(quote_id)
    .fetch_all(&state.db)
    .await?;

    let candidates = rows
        .into_iter()
        .map(|row| {
            let venue_type: String = row.get("venue_type");
            let venue_ref: String = row.get("venue_ref");
            let price: f64 = row.get::<String, _>("price").parse().unwrap_or(0.0);
            let available_amount: f64 = row
                .get::<String, _>("available_amount")
                .parse()
                .unwrap_or(0.0);
            DirectVenueCandidate {
                venue_type,
                venue_ref,
                price,
                available_amount,
            }
        })
        .collect::<Vec<_>>();

    // Build VenueScorerInput from candidates
    let scorer_inputs: Vec<VenueScorerInput> = candidates
        .iter()
        .map(|c| {
            let now = chrono::Utc::now();
            if c.venue_type == "amm" {
                VenueScorerInput {
                    venue_ref: c.venue_ref.clone(),
                    venue_type: VenueType::Amm,
                    best_bid_e7: None,
                    best_ask_e7: None,
                    depth_top_n_e7: None,
                    reserve_a_e7: Some((c.available_amount * 1e7) as i128),
                    reserve_b_e7: Some((c.available_amount * 1e7) as i128),
                    tvl_e7: Some((c.available_amount * 2e7) as i128),
                    last_updated_at: now,
                }
            } else {
                VenueScorerInput {
                    venue_ref: c.venue_ref.clone(),
                    venue_type: VenueType::Sdex,
                    best_bid_e7: None,
                    best_ask_e7: Some((c.price * 1e7) as i128),
                    depth_top_n_e7: Some((c.available_amount * 1e7) as i128),
                    reserve_a_e7: None,
                    reserve_b_e7: None,
                    tvl_e7: None,
                    last_updated_at: now,
                }
            }
        })
        .collect();

    // Health scoring / exclusion policy (defaults match routing `HealthScoringConfig`)
    let health_config = HealthScoringConfig::default();

    let scorer = HealthScorer {
        sdex: SdexScorer {
            staleness_threshold_secs: health_config.staleness_threshold_secs,
            max_spread: 0.05,
            target_depth_e7: 10_000_000_000,
            depth_levels: health_config.depth_levels,
        },
        amm: AmmScorer {
            staleness_threshold_secs: health_config.staleness_threshold_secs,
            min_tvl_threshold_e7: health_config.min_tvl_threshold_e7,
        },
    };

    let scored = scorer.score_venues(&scorer_inputs);

    let policy = ExclusionPolicy {
        thresholds: health_config.thresholds.clone(),
        overrides: OverrideRegistry::from_entries(health_config.overrides.clone()),
    };

    // Apply filter (pass empty edges — we just need diagnostics for this single-hop path)
    let filter = GraphFilter::new(&policy);
    let (_, routing_diagnostics) = filter.filter_edges(&[], &scored);

    // Convert routing diagnostics to API types
    let api_diagnostics = ApiExclusionDiagnostics {
        excluded_venues: routing_diagnostics
            .excluded_venues
            .iter()
            .map(|v| ApiExcludedVenueInfo {
                venue_ref: v.venue_ref.clone(),
                score: v.score,
                signals: v.signals.clone(),
                reason: match &v.reason {
                    stellarroute_routing::health::policy::ExclusionReason::PolicyThreshold {
                        threshold,
                    } => ApiExclusionReason::PolicyThreshold {
                        threshold: *threshold,
                    },
                    stellarroute_routing::health::policy::ExclusionReason::Override => {
                        ApiExclusionReason::Override
                    }
                },
            })
            .collect(),
    };

    let (selected, rationale) = evaluate_single_hop_direct_venues(candidates, amount)?;

    let path = vec![PathStep {
        from_asset: asset_path_to_info(base),
        to_asset: asset_path_to_info(quote),
        price: format!("{:.7}", selected.price),
        source: selected.path_source(),
    }];

    Ok((selected.price, path, rationale, api_diagnostics))
}

#[derive(Debug, Clone)]
struct DirectVenueCandidate {
    venue_type: String,
    venue_ref: String,
    price: f64,
    available_amount: f64,
}

impl DirectVenueCandidate {
    fn comparison_source(&self) -> String {
        format!("{}:{}", self.venue_type, self.venue_ref)
    }

    fn path_source(&self) -> String {
        if self.venue_type == "amm" {
            format!("amm:{}", self.venue_ref)
        } else {
            "sdex".to_string()
        }
    }
}

fn evaluate_single_hop_direct_venues(
    mut candidates: Vec<DirectVenueCandidate>,
    amount: f64,
) -> Result<(DirectVenueCandidate, QuoteRationaleMetadata)> {
    if candidates.is_empty() {
        return Err(ApiError::NoRouteFound);
    }

    candidates.sort_by(|a, b| {
        a.price
            .partial_cmp(&b.price)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.venue_type.cmp(&b.venue_type))
            .then_with(|| a.venue_ref.cmp(&b.venue_ref))
    });

    let compared_venues = candidates
        .iter()
        .map(|candidate| VenueEvaluation {
            source: candidate.comparison_source(),
            price: format!("{:.7}", candidate.price),
            available_amount: format!("{:.7}", candidate.available_amount),
            executable: candidate.available_amount >= amount && candidate.price > 0.0,
        })
        .collect::<Vec<_>>();

    let selected = candidates
        .iter()
        .find(|candidate| candidate.available_amount >= amount && candidate.price > 0.0)
        .cloned()
        .ok_or(ApiError::NoRouteFound)?;

    Ok((
        selected.clone(),
        QuoteRationaleMetadata {
            strategy: "single_hop_direct_venue_comparison".to_string(),
            selected_source: selected.comparison_source(),
            compared_venues,
        },
    ))
}

async fn maybe_invalidate_quote_cache(
    state: &AppState,
    base: &str,
    quote: &str,
    base_id: uuid::Uuid,
    quote_id: uuid::Uuid,
) -> Result<()> {
    let liquidity_revision = get_liquidity_revision(state, base_id, quote_id).await?;

    if let Some(cache) = &state.cache {
        if let Ok(mut cache) = cache.try_lock() {
            let revision_key = cache::keys::liquidity_revision(base, quote);
            let cached_revision = cache.get::<String>(&revision_key).await;

            if cached_revision.as_deref() != Some(liquidity_revision.as_str()) {
                if cached_revision.is_some() {
                    let pattern = cache::keys::quote_pair_pattern(base, quote);
                    let deleted = cache.delete_by_pattern(&pattern).await.unwrap_or(0);
                    debug!(
                        "Liquidity revision changed for {}/{}; invalidated {} quote cache keys",
                        base, quote, deleted
                    );
                }

                let _ = cache
                    .set(
                        &revision_key,
                        &liquidity_revision,
                        std::time::Duration::from_secs(3600),
                    )
                    .await;
            }
        }
    }

    Ok(())
}

async fn get_liquidity_revision(
    state: &AppState,
    base_id: uuid::Uuid,
    quote_id: uuid::Uuid,
) -> Result<String> {
    let row = sqlx::query(
        r#"
        select coalesce(max(source_ledger), 0)::bigint as revision
        from normalized_liquidity
        where (selling_asset_id = $1 and buying_asset_id = $2)
           or (selling_asset_id = $2 and buying_asset_id = $1)
        "#,
    )
    .bind(base_id)
    .bind(quote_id)
    .fetch_one(&state.db)
    .await?;

    let revision: i64 = row.get("revision");
    Ok(revision.to_string())
}

/// Find asset ID in database
async fn find_asset_id(state: &AppState, asset: &AssetPath) -> Result<uuid::Uuid> {
    use sqlx::Row;

    let asset_type = asset.to_asset_type();

    let row = if asset.asset_code == "native" {
        sqlx::query(
            r#"
            select id from assets
            where asset_type = $1
            limit 1
            "#,
        )
        .bind(&asset_type)
        .fetch_optional(&state.db)
        .await?
    } else {
        sqlx::query(
            r#"
            select id from assets
            where asset_type = $1
              and asset_code = $2
              and ($3::text is null or asset_issuer = $3)
            limit 1
            "#,
        )
        .bind(&asset_type)
        .bind(&asset.asset_code)
        .bind(&asset.asset_issuer)
        .fetch_optional(&state.db)
        .await?
    };

    match row {
        Some(row) => Ok(row.get("id")),
        None => Err(ApiError::NotFound(format!(
            "Asset not found: {}",
            asset.asset_code
        ))),
    }
}

/// Convert AssetPath to AssetInfo
fn asset_path_to_info(asset: &AssetPath) -> AssetInfo {
    if asset.asset_code == "native" {
        AssetInfo::native()
    } else {
        AssetInfo::credit(asset.asset_code.clone(), asset.asset_issuer.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        venue_type: &str,
        venue_ref: &str,
        price: f64,
        available_amount: f64,
    ) -> DirectVenueCandidate {
        DirectVenueCandidate {
            venue_type: venue_type.to_string(),
            venue_ref: venue_ref.to_string(),
            price,
            available_amount,
        }
    }

    #[test]
    fn selects_best_executable_direct_venue() {
        let candidates = vec![
            candidate("amm", "pool1", 1.02, 100.0),
            candidate("sdex", "offer2", 1.01, 25.0),
            candidate("sdex", "offer1", 1.00, 75.0),
        ];

        let (selected, rationale) =
            evaluate_single_hop_direct_venues(candidates, 50.0).expect("must select a venue");

        assert_eq!(selected.venue_type, "sdex");
        assert_eq!(selected.venue_ref, "offer1");
        assert_eq!(rationale.selected_source, "sdex:offer1");
        assert_eq!(rationale.compared_venues.len(), 3);
    }

    #[test]
    fn tie_break_is_deterministic_by_venue_then_ref() {
        let candidates = vec![
            candidate("sdex", "offer2", 1.0, 100.0),
            candidate("amm", "pool1", 1.0, 100.0),
            candidate("sdex", "offer1", 1.0, 100.0),
        ];

        let (selected, rationale) =
            evaluate_single_hop_direct_venues(candidates, 10.0).expect("must select a venue");

        assert_eq!(selected.comparison_source(), "amm:pool1");
        assert_eq!(
            rationale
                .compared_venues
                .iter()
                .map(|v| v.source.clone())
                .collect::<Vec<_>>(),
            vec![
                "amm:pool1".to_string(),
                "sdex:offer1".to_string(),
                "sdex:offer2".to_string(),
            ]
        );
    }

    #[test]
    fn insufficient_liquidity_returns_no_route() {
        let candidates = vec![
            candidate("amm", "pool1", 1.0, 5.0),
            candidate("sdex", "offer1", 0.99, 2.0),
        ];

        let result = evaluate_single_hop_direct_venues(candidates, 10.0);
        assert!(matches!(result, Err(ApiError::NoRouteFound)));
    }
}
