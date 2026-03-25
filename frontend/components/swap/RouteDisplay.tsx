"use client";

import { ArrowRight, Info } from "lucide-react";
import { Badge } from "@/components/ui/badge";

interface RouteDisplayProps {
  amountOut: string;
}

export function RouteDisplay({ amountOut }: RouteDisplayProps) {
  return (
    <div className="rounded-xl border border-border/50 p-4 space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <h4 className="text-sm font-medium">Best Route</h4>
          <Info className="h-4 w-4 text-muted-foreground cursor-help" />
        </div>
        <Badge variant="secondary" className="text-xs bg-emerald-500/10 text-emerald-600 hover:bg-emerald-500/20 border-emerald-500/20">
          Optimal
        </Badge>
      </div>

      <div className="flex items-center justify-between bg-muted/50 rounded-lg p-3 overflow-hidden">
        <div className="flex flex-col flex-shrink-0 min-w-[40px]">
          <span className="text-xs font-semibold">XLM</span>
          <span className="text-[10px] text-muted-foreground leading-none">Stellar</span>
        </div>
        <ArrowRight className="h-4 w-4 text-muted-foreground mx-auto flex-shrink-0" />
        <div className="px-2 py-1 bg-background rounded-md border text-xs font-medium shadow-sm flex-shrink-0 text-center mx-1">
          AQUA Pool
        </div>
        <ArrowRight className="h-4 w-4 text-muted-foreground mx-auto flex-shrink-0" />
        <div className="flex flex-col text-right flex-shrink-0 min-w-[60px]">
          <span className="text-xs font-semibold">USDC</span>
          <span className="text-[10px] text-muted-foreground truncate max-w-[80px]" title={`${amountOut} expected`}>{amountOut} exp.</span>
        </div>
      </div>
      
      {/* Alternative routes mocked */}
      <div className="pt-3 border-t border-border/50">
        <h4 className="text-[11px] font-medium text-muted-foreground mb-2 uppercase tracking-wider">Alternative Routes</h4>
        <div className="flex items-center justify-between opacity-60 hover:opacity-100 transition-opacity cursor-pointer p-1 -mx-1 rounded hover:bg-muted/50">
          <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <span className="font-medium">XLM</span>
            <ArrowRight className="h-3 w-3" />
            <span className="border border-border/50 rounded bg-background px-1.5 py-0.5 text-[10px]">SDEX</span>
            <ArrowRight className="h-3 w-3" />
            <span className="font-medium">USDC</span>
          </div>
          <span className="text-xs font-medium text-muted-foreground">≈ {(parseFloat(amountOut) * 0.995).toFixed(4)}</span>
        </div>
      </div>
    </div>
  );
}
