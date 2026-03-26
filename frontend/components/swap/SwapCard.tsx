'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { RotateCcw } from 'lucide-react';
import { PairSelector } from './PairSelector';
import { QuoteSummary } from './QuoteSummary';
import { RouteDisplay } from './RouteDisplay';
import { SlippageControl } from './SlippageControl';
import { SwapCTA } from './SwapCTA';
import { SimulationPanel } from './SimulationPanel';
import { useTradeFormStorage } from '@/hooks/useTradeFormStorage';
import { useState } from 'react';

export function SwapCard() {
  const {
    amount: payAmount,
    setAmount: setPayAmount,
    slippage,
    setSlippage,
    reset,
    isHydrated,
  } = useTradeFormStorage();

  const [receiveAmount, setReceiveAmount] = useState<string>('');
  const [isLoading, setIsLoading] = useState<boolean>(false);

  // Derived state for the button
  const isValidAmount = parseFloat(payAmount) > 0;

  // Simulate quote fetching
  const handlePayAmountChange = (amount: string) => {
    setPayAmount(amount);
    if (parseFloat(amount) > 0) {
      setIsLoading(true);
      setTimeout(() => {
        setReceiveAmount((parseFloat(amount) * 0.98).toFixed(4));
        setIsLoading(false);
      }, 500);
    } else {
      setReceiveAmount('');
    }
  };

  const handleReset = () => {
    reset();
    setReceiveAmount('');
  };

  // Defer render until localStorage has been read to avoid flash of default values
  if (!isHydrated) {
    return (
      <Card className="w-full border shadow-sm">
        <CardHeader className="pb-4">
          <CardTitle className="text-xl font-semibold">Swap</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="h-32 animate-pulse rounded-lg bg-muted" />
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="w-full border shadow-sm">
      <CardHeader className="pb-4">
        <div className="flex items-center justify-between flex-row">
          <CardTitle className="text-xl font-semibold">Swap</CardTitle>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon"
              className="h-11 w-11 rounded-full"
              onClick={handleReset}
              title="Clear form"
            >
              <RotateCcw className="h-4 w-4 text-muted-foreground" />
              <span className="sr-only">Clear form</span>
            </Button>
            <SlippageControl slippage={slippage} onChange={setSlippage} />
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <PairSelector
          payAmount={payAmount}
          onPayAmountChange={handlePayAmountChange}
          receiveAmount={receiveAmount}
        />
        {isValidAmount && !isLoading && receiveAmount && (
          <>
            <SimulationPanel
              payAmount={payAmount}
              expectedOutput={receiveAmount}
              slippage={slippage}
              isLoading={isLoading}
            />
            <QuoteSummary rate="1 XLM ≈ 0.98 USDC" fee="0.01 XLM" priceImpact="< 0.1%" />
            <RouteDisplay amountOut={receiveAmount} />
          </>
        )}
        <SwapCTA
          amount={payAmount}
          isLoading={isLoading}
          hasPair={true}
          onSwap={() => console.log('Swapping...')}
        />
      </CardContent>
    </Card>
  );
}
