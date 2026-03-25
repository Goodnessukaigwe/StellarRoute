export interface Asset {
  asset_type: 'native' | 'credit_alphanum4' | 'credit_alphanum12';
  asset_code?: string;
  asset_issuer?: string;
}

export interface TradingPair {
  base: string;
  counter: string;
  base_asset: string;
  counter_asset: string;
  offer_count: number;
  last_updated?: string;
}

export interface PairsResponse {
  pairs: TradingPair[];
  total: number;
}

export interface OrderbookEntry {
  price: string;
  amount: string;
  total: string;
}

export interface Orderbook {
  base_asset: Asset;
  quote_asset: Asset;
  bids: OrderbookEntry[];
  asks: OrderbookEntry[];
  timestamp: number;
}

export type QuoteType = 'sell' | 'buy';

export interface PathStep {
  from_asset: Asset;
  to_asset: Asset;
  price: string;
  source: string;
}

export interface PriceQuote {
  base_asset: Asset;
  quote_asset: Asset;
  amount: string;
  price: string;
  total: string;
  quote_type: QuoteType;
  path: PathStep[];
  /** Unix timestamp (ms) when this quote was generated */
  timestamp: number;
  /** Unix timestamp (ms) when this quote expires and should be considered stale */
  expires_at?: number;
  /** Unix timestamp (ms) of the underlying data source (e.g., orderbook snapshot) */
  source_timestamp?: number;
  /** Time-to-live in seconds for client-side staleness detection */
  ttl_seconds?: number;
}

/**
 * Configuration for quote staleness detection
 */
export interface QuoteStalenessConfig {
  /** Maximum quote age in seconds before considering stale (default: 30) */
  max_age_seconds: number;
  /** Whether to reject stale quotes on the client side */
  reject_stale?: boolean;
}

/**
 * Default staleness configuration
 */
export const DEFAULT_STALENESS_CONFIG: QuoteStalenessConfig = {
  max_age_seconds: 30,
  reject_stale: false,
};

/**
 * Check if a quote is considered stale
 */
export function isQuoteStale(quote: PriceQuote, config: QuoteStalenessConfig = DEFAULT_STALENESS_CONFIG): boolean {
  const now = Date.now();
  const ageMs = now - quote.timestamp;
  const maxAgeMs = config.max_age_seconds * 1000;
  return ageMs > maxAgeMs;
}

/**
 * Check if a quote has expired based on its expires_at field
 */
export function isQuoteExpired(quote: PriceQuote): boolean {
  if (!quote.expires_at) return false;
  return Date.now() > quote.expires_at;
}

/**
 * Get remaining time until quote expires (in seconds), or null if no expiry
 */
export function getTimeUntilExpiry(quote: PriceQuote): number | null {
  if (!quote.expires_at) return null;
  const remaining = quote.expires_at - Date.now();
  return remaining > 0 ? Math.floor(remaining / 1000) : 0;
}

export interface HealthStatus {
  status: 'healthy' | 'unhealthy';
  version: string;
  timestamp: string;
  components: Record<string, string>;
}

export interface ApiError {
  error: string;
  message: string;
  details?: unknown;
}
