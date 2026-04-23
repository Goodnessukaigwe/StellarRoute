//! OpenAPI documentation

use utoipa::OpenApi;

use crate::models::{
    AssetInfo, CacheMetricsResponse, DependenciesHealthResponse, ErrorResponse, HealthResponse,
    OrderbookLevel, OrderbookResponse, PairsResponse, PathStep, QuoteRationaleMetadata,
    QuoteResponse, RouteResponse, TradingPair, VenueEvaluation,
};

/// OpenAPI documentation
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::health::health_check,
        crate::routes::health::dependency_health,
        crate::routes::metrics::cache_metrics,
        crate::routes::pairs::list_pairs,
        crate::routes::pairs::list_markets,
        crate::routes::orderbook::get_orderbook,
        crate::routes::quote::get_quote,
        crate::routes::quote::get_route,
    ),
    components(schemas(
        HealthResponse,
        DependenciesHealthResponse,
        CacheMetricsResponse,
        PairsResponse,
        TradingPair,
        AssetInfo,
        OrderbookResponse,
        OrderbookLevel,
        QuoteResponse,
        RouteResponse,
        QuoteRationaleMetadata,
        VenueEvaluation,
        PathStep,
        ErrorResponse,
    )),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "trading", description = "Trading and market data endpoints"),
    ),
    info(
        title = "StellarRoute API",
        version = "0.1.0",
        description = "REST API for DEX aggregation on Stellar Network. Clients may send an optional X-Request-ID header for support correlation; the API echoes the same header in every response.",
        contact(
            name = "StellarRoute",
            url = "https://github.com/stellarroute/stellarroute"
        ),
        license(
            name = "MIT",
        ),
    ),
)]
pub struct ApiDoc;
