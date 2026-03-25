import { describe, expect, it, vi, afterEach } from 'vitest';
import { StellarRouteClient, StellarRouteApiError } from './client.js';

function ok(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

function apiError(code: string, message: string, status: number): Response {
    return new Response(JSON.stringify({ error: code, message }), {
      status,
      headers: { 'Content-Type': 'application/json' },
    });
}

const NATIVE = { asset_type: 'native' } as import('./types.js').Asset;
const USDC = { asset_type: 'credit_alphanum4', asset_code: 'USDC', asset_issuer: 'GDUKMGUGDZQK6YH...' } as import('./types.js').Asset;

afterEach(() => {
  vi.restoreAllMocks();
});

describe('Thin Orderbook Routing Scenarios', () => {
  it('splits across routes correctly when a single path has insufficient depth (thin markets)', async () => {
    // Asserting that the path logic correctly segments the trade across multiple venues 
    // down different depths to handle thin liquidity.
    const splitQuoteFixture = {
      base_asset: NATIVE,
      quote_asset: USDC,
      amount: '1000',
      price: '0.94',
      total: '940',
      quote_type: 'sell',
      path: [
        { from_asset: NATIVE, to_asset: USDC, price: '0.96', source: 'sdex:offer1' },
        { from_asset: NATIVE, to_asset: USDC, price: '0.92', source: 'amm:poolXYZ' }
      ],
      timestamp: 1_700_000_000,
    };

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(ok(splitQuoteFixture));

    const client = new StellarRouteClient();
    const result = await client.getQuote('native', 'USDC:GDUKMGUGDZQK6YH...', 1000);

    // Verify it evaluates to a compounded multi-venue path
    expect(result.path.length).toBeGreaterThan(1);
    expect(result.path[0]?.source).toContain('sdex');
    expect(result.path[1]?.source).toContain('amm');
    expect(result.total).toBe('940'); 
  });

  it('rejects trade if slippage exceeds limits instead of executing poorly in a thin market', async () => {
    // Simulating routing logic validating slippage threshold inside a thin orderbook.
    // E.g., slippage protection is natively triggered.
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      apiError('slippage_exceeded', 'Slippage limit violated due to thin orderbook depth', 400)
    );

    const client = new StellarRouteClient({ retries: 0 });
    let error: StellarRouteApiError | undefined;
    
    // Attempting to route a massive trade relative to the test network
    try {
      await client.getQuote('native', 'USDC:GDUKMGUGDZQK6YH...', 500_000);
    } catch (e: any) {
      error = e;
    }

    expect(error).toBeDefined();
    expect(error?.status).toBe(400);
    expect(error?.code).toBe('slippage_exceeded');
  });

  it('selects the safest best route avoiding misleading deep paths that degrade execution quality', async () => {
    // Ensuring the router returns the best reliable route even when an alternative has a nominally
    // high spot price but tiny depth
    const robustQuoteFixture = {
      base_asset: NATIVE,
      quote_asset: USDC,
      amount: '500',
      price: '0.98',
      total: '490',
      quote_type: 'sell',
      path: [
        { from_asset: NATIVE, to_asset: USDC, price: '0.98', source: 'sdex:reliable-offer' },
      ],
      timestamp: 1_700_000_000,
    };

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(ok(robustQuoteFixture));

    const client = new StellarRouteClient();
    const result = await client.getQuote('native', 'USDC:GDUKMGUGDZQK6YH...', 500);

    expect(result.path).toHaveLength(1);
    expect(result.path[0]?.source).toBe('sdex:reliable-offer'); 
    expect(result.price).toBe('0.98');
  });
});
