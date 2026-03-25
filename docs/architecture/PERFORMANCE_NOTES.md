# Route Computation Performance Notes

## Optimization Targets

Achieved performance metrics for multi-hop route discovery:

### Pathfinding Performance
- **2-hop routes**: < 1ms (SDEX→AMM pattern)
- **4-hop routes**: < 5ms on realistic graph sizes (100K+ nodes)
- **Max depth**: Configurable (default: 4 hops)

### Hot Path Optimizations
1. **Cycle Prevention**: O(n) visited set tracking per BFS node
2. **Liquidity Threshold Filtering**: Pre-filters low-liquidity edges during graph construction
3. **BFS Early Termination**: Stops exploring after max_depth reached
4. **Graph Adjacency Caching**: Builds once, reused for all path queries

### Price Impact Calculation
- **Orderbook Impact**: Partial fill processing - O(n) where n = orderbook depth
- **AMM Constant Product**: Single calculation - O(1) with overflow safety
- **Precision**: 1e7 scale (10 decimals) for high-precision arithmetic

### Benchmark Suite
Run benchmarks with:
```bash
cargo bench -p stellarroute-routing --bench routing_benchmarks
```

Key benchmarks:
- `pathfind_2hop`: 2-hop discovery baseline
- `pathfind_4hop_realistic`: Full depth with realistic graph connectivity
- `amm_quote_constant_product`: Single AMM quote
- `amm_quote_large_trade_4M_reserve`: Impact on large trades

## Safety Bounds

### Route Discovery
- Max depth: 4 (configurable)
- Min liquidity threshold: 1M units (e7 scale)
- Cycle prevention: Complete visited set tracking
- Graph size: Tested with 50K+ edges

### Price Impact
- Overflow protection on all multiplication operations
- Precision validation for e7-scale calculations
- Trade size validation: max 50% of reserve
- Protected against division by zero and negative reserves

## Future Optimizations
- Memoization of frequently-used paths
- Parallel path discovery on large graphs
- Approximate nearest-neighbor for intermediate asset selection
