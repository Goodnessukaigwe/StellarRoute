import type {
  HealthStatus,
  Orderbook,
  PairsResponse,
  PathStep,
  PriceQuote,
  QuoteStalenessConfig,
  QuoteType,
} from './types.js';
import { DEFAULT_STALENESS_CONFIG, isQuoteStale, isQuoteExpired } from './types.js';

const DEFAULT_TIMEOUT_MS = 10_000;

export class StellarRouteApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    message: string,
    public readonly details?: unknown,
  ) {
    super(message);
    this.name = 'StellarRouteApiError';
  }
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

interface FetchOptions {
  signal?: AbortSignal;
}

export class StellarRouteClient {
  private readonly baseUrl: string;

  constructor(baseUrl = 'http://localhost:8080') {
    this.baseUrl = baseUrl.replace(/\/$/, '');
  }

  private async request<T>(
    path: string,
    opts: FetchOptions = {},
    retries = 2,
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), DEFAULT_TIMEOUT_MS);
    opts.signal?.addEventListener('abort', () => controller.abort());

    try {
      const response = await fetch(url, {
        headers: { Accept: 'application/json' },
        signal: controller.signal,
      });

      if (!response.ok) {
        let code = 'unknown_error';
        let message = `HTTP ${response.status}`;
        let details: unknown;

        try {
          const body = await response.json();
          code = body.error ?? code;
          message = body.message ?? message;
          details = body.details;
        } catch {
          // Keep default values for non-JSON response bodies
        }

        if ((response.status === 429 || response.status >= 500) && retries > 0) {
          const retryAfter =
            Number(response.headers.get('Retry-After') ?? 1) * 1_000;
          await sleep(retryAfter || 1_000 * (3 - retries));
          return this.request<T>(path, opts, retries - 1);
        }

        throw new StellarRouteApiError(response.status, code, message, details);
      }

      return response.json() as Promise<T>;
    } catch (err) {
      if (err instanceof StellarRouteApiError) throw err;
      if (retries > 0) {
        await sleep(500 * (3 - retries));
        return this.request<T>(path, opts, retries - 1);
      }

      const message = err instanceof Error ? err.message : 'Network error';
      throw new StellarRouteApiError(0, 'network_error', message);
    } finally {
      clearTimeout(timer);
    }
  }

  getHealth(opts?: FetchOptions): Promise<HealthStatus> {
    return this.request<HealthStatus>('/health', opts);
  }

  getPairs(opts?: FetchOptions): Promise<PairsResponse> {
    return this.request<PairsResponse>('/api/v1/pairs', opts);
  }

  getOrderbook(
    base: string,
    quote: string,
    opts?: FetchOptions,
  ): Promise<Orderbook> {
    const path = `/api/v1/orderbook/${encodeURIComponent(base)}/${encodeURIComponent(quote)}`;
    return this.request<Orderbook>(path, opts);
  }

  getQuote(
    base: string,
    quote: string,
    amount?: number,
    type: QuoteType = 'sell',
    opts?: FetchOptions,
  ): Promise<PriceQuote> {
    const params = new URLSearchParams({ quote_type: type });
    if (amount !== undefined) params.set('amount', String(amount));
    const path = `/api/v1/quote/${encodeURIComponent(base)}/${encodeURIComponent(quote)}?${params}`;
    return this.request<PriceQuote>(path, opts);
  }

  /**
   * Get a quote with staleness validation.
   * Throws if the quote is stale or expired based on the provided config.
   */
  async getQuoteWithValidation(
    base: string,
    quote: string,
    amount?: number,
    type: QuoteType = 'sell',
    stalenessConfig: QuoteStalenessConfig = DEFAULT_STALENESS_CONFIG,
    opts?: FetchOptions,
  ): Promise<PriceQuote> {
    const quoteResponse = await this.getQuote(base, quote, amount, type, opts);
    
    // Check if expired (server-provided expiry)
    if (isQuoteExpired(quoteResponse)) {
      throw new StellarRouteApiError(
        0,
        'quote_expired',
        'Quote has expired based on server-provided expiry time',
        { expires_at: quoteResponse.expires_at }
      );
    }
    
    // Check if stale (client-side staleness detection)
    if (stalenessConfig.reject_stale && isQuoteStale(quoteResponse, stalenessConfig)) {
      throw new StellarRouteApiError(
        0,
        'quote_stale',
        `Quote is stale (older than ${stalenessConfig.max_age_seconds} seconds)`,
        { timestamp: quoteResponse.timestamp, max_age_seconds: stalenessConfig.max_age_seconds }
      );
    }
    
    return quoteResponse;
  }

  async getRoutes(
    base: string,
    quote: string,
    amount?: number,
    type: QuoteType = 'sell',
    opts?: FetchOptions,
  ): Promise<PathStep[]> {
    const quoteResponse = await this.getQuote(base, quote, amount, type, opts);
    return quoteResponse.path;
  }
}
