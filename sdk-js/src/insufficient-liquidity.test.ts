import { describe, expect, it, vi, afterEach } from 'vitest';
import { StellarRouteClient, StellarRouteApiError } from './client.js';

// ── Fixtures ──────────────────────────────────────────────────────────────────
const NATIVE = { asset_type: 'native' } as import('./types.js').Asset;
const USDC = { asset_type: 'credit_alphanum4', asset_code: 'USDC', asset_issuer: 'GDUKMGUGDZQK6YH...' } as import('./types.js').Asset;

function apiError(code: string, message: string, status: number): Response {
  return new Response(JSON.stringify({ error: code, message }), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('Insufficient Liquidity Scenarios', () => {
  it('returns an error when no route satisfies the trade size', async () => {
    // Simulate a scenario where the requested amount exceeds all available path liquidity.
    // The routing logic (mocked backend here) correctly identifies a path cannot be formed.
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      apiError('not_found', 'Insufficient liquidity for this trade size', 404)
    );

    const client = new StellarRouteClient({ retries: 0 });
    const hugeTradeSize = 10_000_000_000;

    let error: StellarRouteApiError | undefined;
    try {
      await client.getQuote('native', 'USDC:GDUKMGUGDZQK6YH...', hugeTradeSize);
    } catch (e: any) {
      error = e;
    }

    expect(error).toBeDefined();
    expect(error?.status).toBe(404);
    expect(error?.code).toBe('not_found');
    // Ensure clear, actionable errors
    expect(error?.message).toMatch(/Insufficient liquidity/i);
    expect(error?.isNetworkError()).toBe(false); // Should be a valid HTTP response, not a crash or network error
  });

  it('rejects trade when available liquidity is below minimum operational thresholds', async () => {
    // Simulate routing logic explicitly rejecting a request due to liquidity dropping 
    // too severely during path execution calculation.
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      apiError('validation_error', 'Liquidity below minimum thresholds', 400)
    );

    const client = new StellarRouteClient({ retries: 0 });
    
    let error: StellarRouteApiError | undefined;
    try {
      await client.getQuote('native', 'USDC:GDUKMGUGDZQK6YH...', 1);
    } catch (e: any) {
      error = e;
    }

    expect(error).toBeDefined();
    expect(error?.status).toBe(400);
    expect(error?.code).toBe('validation_error');
    expect(error?.message).toMatch(/Liquidity below minimum thresholds/i);
  });
});
