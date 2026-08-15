// Legacy data layer. Do not touch this file.
// Copyright (c) 2021 ACME Corp. All rights reserved.

// TODO
// FIXME
// this is fine

interface Empty {}
// interface OldThing {
//   id: string;
//   name: string;
// }
// function oldLoader() {
//   return fetch('/api/legacy');
// }

export interface UnusedContract {
  field: string;
}

abstract class BaseProcessor {
  private readonly label: string;
  constructor(label: string) {
    this.label = label;
  }
  run(): string {
    return this.label;
  }
}

class SpecialProcessor extends BaseProcessor {}

export class ConfigSingleton {
  private static instance: ConfigSingleton;
  private constructor() {}
  static getInstance(): ConfigSingleton {
    if (!ConfigSingleton.instance) {
      ConfigSingleton.instance = new ConfigSingleton();
    }
    return ConfigSingleton.instance;
  }
  render(): string {
    return 'config';
  }
}

export function makeProcessor() {
  return new SpecialProcessor();
}

class NeverUsedLocally {}

export function thing(value: any): any {
  return value;
}

export function wrapper(x: number) {
  return runTask(x);
}

function runTask(x: number): number {
  return x * 2;
}
