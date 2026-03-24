import { describe, expect, it, vi, afterEach } from 'vitest';
import { StellarRouteClient } from './client.js';
import type { PriceQuote } from './types.js';

afterEach(() => {
  vi.restoreAllMocks();
});

const sampleQuote: PriceQuote = {
  base_asset: { asset_type: 'native' },
  quote_asset: {
    asset_type: 'credit_alphanum4',
    asset_code: 'USDC',
    asset_issuer: 'GDUKMGUGDZQK6YH...',
  },
  amount: '100',
  price: '0.99',
  total: '99',
  quote_type: 'sell',
  path: [
    {
      from_asset: { asset_type: 'native' },
      to_asset: {
        asset_type: 'credit_alphanum4',
        asset_code: 'USDC',
        asset_issuer: 'GDUKMGUGDZQK6YH...',
      },
      price: '0.99',
      source: 'sdex',
    },
  ],
  timestamp: 1_717_171_717,
};

describe('StellarRouteClient', () => {
  it('uses configurable base URL in getQuote requests', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(new Response(JSON.stringify(sampleQuote), { status: 200 }));

    const client = new StellarRouteClient('https://api.example.com/');
    const quote = await client.getQuote('native', 'USDC:GDUKMGUGDZQK6YH...', 100);

    expect(quote).toEqual(sampleQuote);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      'https://api.example.com/api/v1/quote/native/USDC%3AGDUKMGUGDZQK6YH...?quote_type=sell&amount=100',
    );
  });

  it('builds buy quotes with explicit quote type', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(new Response(JSON.stringify({ ...sampleQuote, quote_type: 'buy' }), { status: 200 }));

    const client = new StellarRouteClient();
    await client.getQuote('native', 'USDC:GDUKMGUGDZQK6YH...', 55, 'buy');

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      'http://localhost:8080/api/v1/quote/native/USDC%3AGDUKMGUGDZQK6YH...?quote_type=buy&amount=55',
    );
  });

  it('returns path steps from getRoutes', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify(sampleQuote), { status: 200 }),
    );

    const client = new StellarRouteClient();
    const routes = await client.getRoutes('native', 'USDC:GDUKMGUGDZQK6YH...', 100);

    expect(routes).toEqual(sampleQuote.path);
    expect(routes).toHaveLength(1);
  });
});
