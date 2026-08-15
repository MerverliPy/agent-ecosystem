# fixture-clean

A deliberately clean fixture repo used by SlopGate tests. Every name is specific,
every helper is used, comments explain intent rather than restating the code.

## Layout

- `src/config.ts` — pricing configuration and currency formatting.
- `src/cart.ts` — line-item totals, tax and receipt rendering.
- `src/index.ts` — checkout entry point.

## Running

```bash
node src/index.ts
```
