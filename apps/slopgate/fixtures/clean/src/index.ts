// Checkout entry point: totals, taxes and receipt rendering.
import { renderReceipt } from './cart.ts';
import { DEFAULT_CONFIG } from './config.ts';

const items = [
  { name: 'Notebook', unitPriceCents: 1299, quantity: 2 },
  { name: 'Pen', unitPriceCents: 199, quantity: 4 },
];

const receipt = renderReceipt(items, DEFAULT_CONFIG);
console.log(receipt);
