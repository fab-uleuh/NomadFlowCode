import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { homedir } from 'node:os';
import { parse as parseToml } from 'smol-toml';
import type { Server } from '../types.js';

export interface NomadFlowConfig {
  paths: {
    base_dir: string;
  };
  tmux: {
    session: string;
  };
  ttyd: {
    port: number;
  };
  api: {
    port: number;
    host: string;
  };
  auth: {
    secret: string;
  };
  /** Servers defined in cli-servers.json */
  servers: Server[];
}

const DEFAULT_CONFIG: NomadFlowConfig = {
  paths: { base_dir: '~/.nomadflowcode' },
  tmux: { session: 'nomadflow' },
  ttyd: { port: 7681 },
  api: { port: 8080, host: '0.0.0.0' },
  auth: { secret: '' },
  servers: [],
};

function expandHome(p: string): string {
  return p.startsWith('~') ? p.replace('~', homedir()) : p;
}

export function getBaseDir(config: NomadFlowConfig): string {
  return expandHome(config.paths.base_dir);
}

export function loadConfig(): NomadFlowConfig {
  const configPath = join(homedir(), '.nomadflowcode', 'config.toml');

  let config = { ...DEFAULT_CONFIG };

  if (existsSync(configPath)) {
    try {
      const raw = readFileSync(configPath, 'utf-8');
      const parsed = parseToml(raw) as Record<string, any>;

      config = {
        paths: { ...DEFAULT_CONFIG.paths, ...parsed.paths },
        tmux: { ...DEFAULT_CONFIG.tmux, ...parsed.tmux },
        ttyd: { ...DEFAULT_CONFIG.ttyd, ...parsed.ttyd },
        api: { ...DEFAULT_CONFIG.api, ...parsed.api },
        auth: { ...DEFAULT_CONFIG.auth, ...parsed.auth },
        servers: [],
      };
    } catch {
      // Use defaults if parse fails
    }
  }

  // Load servers from cli-servers.json
  config.servers = loadServers(config);

  return config;
}

function loadServers(config: NomadFlowConfig): Server[] {
  const serversPath = join(getBaseDir(config), 'cli-servers.json');

  // Always include a localhost entry derived from config
  const localhost: Server = {
    id: 'localhost',
    name: 'localhost',
    apiUrl: `http://localhost:${config.api.port}`,
    ttydUrl: `http://localhost:${config.ttyd.port}`,
    authToken: config.auth.secret || undefined,
  };

  if (existsSync(serversPath)) {
    try {
      const raw = readFileSync(serversPath, 'utf-8');
      const servers: Server[] = JSON.parse(raw);
      // Ensure localhost is always present
      const hasLocalhost = servers.some(s => s.id === 'localhost');
      return hasLocalhost ? servers : [localhost, ...servers];
    } catch {
      return [localhost];
    }
  }

  return [localhost];
}
