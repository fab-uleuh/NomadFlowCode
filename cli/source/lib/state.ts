import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import type { CliState } from './types.js';
import { getBaseDir, type NomadFlowConfig } from './config.js';

function getStatePath(config: NomadFlowConfig): string {
  return join(getBaseDir(config), 'cli-state.json');
}

export function loadState(config: NomadFlowConfig): CliState {
  const statePath = getStatePath(config);

  if (existsSync(statePath)) {
    try {
      const raw = readFileSync(statePath, 'utf-8');
      return JSON.parse(raw) as CliState;
    } catch {
      return {};
    }
  }

  return {};
}

export function saveState(config: NomadFlowConfig, state: CliState): void {
  const baseDir = getBaseDir(config);
  const statePath = getStatePath(config);

  if (!existsSync(baseDir)) {
    mkdirSync(baseDir, { recursive: true });
  }

  writeFileSync(statePath, JSON.stringify(state, null, 2));
}
