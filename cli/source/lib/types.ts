import type { Feature } from '../types.js';

export interface CliFeature extends Feature {
  paneCommand?: string;
}

export interface CliState {
  lastServer?: string;
  lastRepo?: string;
  lastFeature?: string;
  lastAttached?: number;
}

export type WizardStep = 'resume' | 'server' | 'repo' | 'feature' | 'attach';
