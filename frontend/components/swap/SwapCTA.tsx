"use client";

import { Button } from "@/components/ui/button";
import { Loader2 } from "lucide-react";

interface SwapCTAProps {
  amount: string;
  isLoading: boolean;
  hasPair: boolean;
  onSwap: () => void;
}

export function SwapCTA({ amount, isLoading, hasPair, onSwap }: SwapCTAProps) {
  const numAmount = parseFloat(amount || "0");

  let label = "Review Swap";
  let disabled = false;

  if (!hasPair) {
    label = "Select tokens";
    disabled = true;
  } else if (!amount || isNaN(numAmount) || numAmount <= 0) {
    label = "Enter amount";
    disabled = true;
  } else if (isLoading) {
    label = "Loading quote...";
    disabled = true;
  }

  return (
    <Button 
      className="w-full h-14 text-lg font-medium shadow-md transition-all active:scale-[0.98] mt-2" 
      size="lg"
      disabled={disabled}
      onClick={onSwap}
    >
      {isLoading && <Loader2 className="mr-2 h-5 w-5 animate-spin" />}
      {label}
    </Button>
  );
}
