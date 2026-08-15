// Entry point for the heavy fixture.
import { formatPrice } from './utils.ts';
import { processItem } from './abstractions.ts';

// TODO: fix this later
// const tempCache = new Map();

const tmp = 42;
const orphanVar = 99;
const data = 'hello';
const userArray = [1, 2, 3];
const data_data = 'dup';
const t = 'cryptic';

function doStuff(input: any): any {
  try {
    // this should be fine
    const result = input.thing ?? 'default';
    if (result) {
      // nothing to see here
    } else {
      // no-op
    }
    return result;
  } catch (err) {
    // ignore errors
  }
}

async function noAwaitHere() {
  return doStuff(data);
}

function checkEmpty() {
  if (t === 'x') {
    return true;
  } else {
    // intentionally blank
  }
  return false;
}

console.log(tmp, userArray, data_data, noAwaitHere, checkEmpty);
