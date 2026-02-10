import AsyncStorage from '@react-native-async-storage/async-storage';
import React, { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';

import type { Server, Repository, Feature } from '@shared';
import type { AppSettings, TerminalShortcut } from '../types';

/** Derive ttydUrl from apiUrl (same host, port 7681). */
function deriveTtydUrl(apiUrl: string): string {
  try {
    const url = new URL(apiUrl);
    url.port = '7681';
    url.pathname = '/';
    return url.toString().replace(/\/$/, '');
  } catch {
    return 'http://localhost:7681';
  }
}

/**
 * Migrate old server formats to current format.
 * Handles: old 'url' (WebSocket) field, old 'ttydUrl' field.
 */
function migrateServer(server: any): Server {
  const migrated: Server = {
    id: server.id,
    name: server.name,
    apiUrl: server.apiUrl,
    ttydUrl: server.ttydUrl,
    authToken: server.authToken,
    lastConnected: server.lastConnected,
  };

  // Derive apiUrl from old fields if missing
  if (!migrated.apiUrl) {
    const ttydUrl = server.ttydUrl || '';
    if (ttydUrl) {
      try {
        const url = new URL(ttydUrl);
        url.port = url.port === '7681' ? '8080' : url.port;
        migrated.apiUrl = url.toString().replace(/\/$/, '');
      } catch {
        migrated.apiUrl = ttydUrl.replace(':7681', ':8080');
      }
    } else {
      const oldUrl = server.url || '';
      if (oldUrl) {
        const httpUrl = oldUrl
          .replace('wss://', 'https://')
          .replace('ws://', 'http://')
          .replace('/ws', '');
        try {
          const url = new URL(httpUrl);
          url.port = url.port === '7681' ? '8080' : url.port;
          migrated.apiUrl = url.toString().replace(/\/$/, '');
        } catch {
          migrated.apiUrl = httpUrl.replace(':7681', ':8080');
        }
      } else {
        migrated.apiUrl = 'http://localhost:8080';
      }
    }
  }

  // Ensure ttydUrl is set (derive from apiUrl if missing)
  if (!migrated.ttydUrl) {
    migrated.ttydUrl = deriveTtydUrl(migrated.apiUrl!);
  }

  return migrated;
}

interface LastSelection {
  serverId?: string;
  repoPath?: string;
  featureName?: string;
}

interface StorageContextType {
  servers: Server[];
  recentRepos: Repository[];
  recentFeatures: Feature[];
  lastSelection: LastSelection;
  settings: AppSettings;
  terminalShortcuts: TerminalShortcut[];
  isLoading: boolean;

  // Server operations
  addServer: (server: Omit<Server, 'id'>) => Promise<Server>;
  updateServer: (id: string, updates: Partial<Server>) => Promise<void>;
  deleteServer: (id: string) => Promise<void>;
  getServer: (id: string) => Server | undefined;

  // Recent items
  addRecentRepo: (repo: Repository) => Promise<void>;
  addRecentFeature: (feature: Feature) => Promise<void>;

  // Last selection
  saveLastSelection: (selection: LastSelection) => Promise<void>;

  // Settings
  updateSettings: (updates: Partial<AppSettings>) => Promise<void>;

  // Terminal shortcuts
  addTerminalShortcut: (shortcut: Omit<TerminalShortcut, 'id'>) => Promise<TerminalShortcut>;
  updateTerminalShortcut: (id: string, updates: Partial<TerminalShortcut>) => Promise<void>;
  deleteTerminalShortcut: (id: string) => Promise<void>;

  // Utility
  clearAllData: () => Promise<void>;
}

const defaultSettings: AppSettings = {
  defaultAiAgent: 'claude',
  autoLaunchAgent: true,
  tmuxSessionPrefix: 'nomadflow',
  theme: 'dark',
  fontSize: 30,
  autoReconnect: true,
  reconnectDelay: 3000,
  maxReconnectAttempts: 5,
};

const STORAGE_KEYS = {
  SERVERS: '@nomadflow_servers',
  RECENT_REPOS: '@nomadflow_recent_repos',
  RECENT_FEATURES: '@nomadflow_recent_features',
  LAST_SELECTION: '@nomadflow_last_selection',
  SETTINGS: '@nomadflow_settings',
  TERMINAL_SHORTCUTS: '@nomadflow_terminal_shortcuts',
};

const StorageContext = createContext<StorageContextType | undefined>(undefined);

interface StorageProviderProps {
  children: ReactNode;
}

export function StorageProvider({ children }: StorageProviderProps) {
  const [servers, setServers] = useState<Server[]>([]);
  const [recentRepos, setRecentRepos] = useState<Repository[]>([]);
  const [recentFeatures, setRecentFeatures] = useState<Feature[]>([]);
  const [lastSelection, setLastSelection] = useState<LastSelection>({});
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [terminalShortcuts, setTerminalShortcuts] = useState<TerminalShortcut[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    loadAllData();
  }, []);

  const loadAllData = async () => {
    try {
      const [serversData, recentReposData, recentFeaturesData, lastSelectionData, settingsData, shortcutsData] =
        await Promise.all([
          AsyncStorage.getItem(STORAGE_KEYS.SERVERS),
          AsyncStorage.getItem(STORAGE_KEYS.RECENT_REPOS),
          AsyncStorage.getItem(STORAGE_KEYS.RECENT_FEATURES),
          AsyncStorage.getItem(STORAGE_KEYS.LAST_SELECTION),
          AsyncStorage.getItem(STORAGE_KEYS.SETTINGS),
          AsyncStorage.getItem(STORAGE_KEYS.TERMINAL_SHORTCUTS),
        ]);

      if (serversData) {
        const parsed = JSON.parse(serversData);
        const migrated = parsed.map(migrateServer);
        setServers(migrated);
        // Save migrated servers back to storage
        if (JSON.stringify(parsed) !== JSON.stringify(migrated)) {
          await AsyncStorage.setItem(STORAGE_KEYS.SERVERS, JSON.stringify(migrated));
        }
      }
      if (recentReposData) setRecentRepos(JSON.parse(recentReposData));
      if (recentFeaturesData) setRecentFeatures(JSON.parse(recentFeaturesData));
      if (lastSelectionData) setLastSelection(JSON.parse(lastSelectionData));
      if (settingsData) setSettings({ ...defaultSettings, ...JSON.parse(settingsData) });
      if (shortcutsData) setTerminalShortcuts(JSON.parse(shortcutsData));
    } catch (error) {
      console.error('Failed to load storage data:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const generateId = (): string => {
    return Date.now().toString(36) + Math.random().toString(36).substr(2);
  };

  const addServer = useCallback(
    async (serverData: Omit<Server, 'id'>): Promise<Server> => {
      const newServer: Server = {
        ...serverData,
        id: generateId(),
      };
      const updatedServers = await new Promise<Server[]>((resolve) => {
        setServers((prev) => {
          const updated = [...prev, newServer];
          resolve(updated);
          return updated;
        });
      });
      await AsyncStorage.setItem(STORAGE_KEYS.SERVERS, JSON.stringify(updatedServers));
      return newServer;
    },
    []
  );

  const updateServer = useCallback(
    async (id: string, updates: Partial<Server>): Promise<void> => {
      const updatedServers = await new Promise<Server[]>((resolve) => {
        setServers((prev) => {
          const updated = prev.map((s) => (s.id === id ? { ...s, ...updates } : s));
          resolve(updated);
          return updated;
        });
      });
      await AsyncStorage.setItem(STORAGE_KEYS.SERVERS, JSON.stringify(updatedServers));
    },
    []
  );

  const deleteServer = useCallback(
    async (id: string): Promise<void> => {
      const updatedServers = await new Promise<Server[]>((resolve) => {
        setServers((prev) => {
          const updated = prev.filter((s) => s.id !== id);
          resolve(updated);
          return updated;
        });
      });
      await AsyncStorage.setItem(STORAGE_KEYS.SERVERS, JSON.stringify(updatedServers));
    },
    []
  );

  const getServer = useCallback(
    (id: string): Server | undefined => {
      return servers.find((s) => s.id === id);
    },
    [servers]
  );

  const addRecentRepo = useCallback(
    async (repo: Repository): Promise<void> => {
      const updatedRepo = { ...repo, lastAccessed: Date.now() };
      const filtered = recentRepos.filter((r) => r.path !== repo.path);
      const updated = [updatedRepo, ...filtered].slice(0, 10);
      setRecentRepos(updated);
      await AsyncStorage.setItem(STORAGE_KEYS.RECENT_REPOS, JSON.stringify(updated));
    },
    [recentRepos]
  );

  const addRecentFeature = useCallback(
    async (feature: Feature): Promise<void> => {
      const filtered = recentFeatures.filter(
        (f) => !(f.name === feature.name && f.worktreePath === feature.worktreePath)
      );
      const updated = [feature, ...filtered].slice(0, 20);
      setRecentFeatures(updated);
      await AsyncStorage.setItem(STORAGE_KEYS.RECENT_FEATURES, JSON.stringify(updated));
    },
    [recentFeatures]
  );

  const saveLastSelection = useCallback(
    async (selection: LastSelection): Promise<void> => {
      const updated = { ...lastSelection, ...selection };
      setLastSelection(updated);
      await AsyncStorage.setItem(STORAGE_KEYS.LAST_SELECTION, JSON.stringify(updated));
    },
    [lastSelection]
  );

  const updateSettings = useCallback(
    async (updates: Partial<AppSettings>): Promise<void> => {
      const updated = { ...settings, ...updates };
      setSettings(updated);
      await AsyncStorage.setItem(STORAGE_KEYS.SETTINGS, JSON.stringify(updated));
    },
    [settings]
  );

  const addTerminalShortcut = useCallback(
    async (shortcutData: Omit<TerminalShortcut, 'id'>): Promise<TerminalShortcut> => {
      const newShortcut: TerminalShortcut = { ...shortcutData, id: generateId() };
      const updated = await new Promise<TerminalShortcut[]>((resolve) => {
        setTerminalShortcuts((prev) => {
          const next = [...prev, newShortcut];
          resolve(next);
          return next;
        });
      });
      await AsyncStorage.setItem(STORAGE_KEYS.TERMINAL_SHORTCUTS, JSON.stringify(updated));
      return newShortcut;
    },
    []
  );

  const updateTerminalShortcut = useCallback(
    async (id: string, updates: Partial<TerminalShortcut>): Promise<void> => {
      const updated = await new Promise<TerminalShortcut[]>((resolve) => {
        setTerminalShortcuts((prev) => {
          const next = prev.map((s) => (s.id === id ? { ...s, ...updates } : s));
          resolve(next);
          return next;
        });
      });
      await AsyncStorage.setItem(STORAGE_KEYS.TERMINAL_SHORTCUTS, JSON.stringify(updated));
    },
    []
  );

  const deleteTerminalShortcut = useCallback(
    async (id: string): Promise<void> => {
      const updated = await new Promise<TerminalShortcut[]>((resolve) => {
        setTerminalShortcuts((prev) => {
          const next = prev.filter((s) => s.id !== id);
          resolve(next);
          return next;
        });
      });
      await AsyncStorage.setItem(STORAGE_KEYS.TERMINAL_SHORTCUTS, JSON.stringify(updated));
    },
    []
  );

  const clearAllData = useCallback(async (): Promise<void> => {
    await AsyncStorage.multiRemove(Object.values(STORAGE_KEYS));
    setServers([]);
    setRecentRepos([]);
    setRecentFeatures([]);
    setLastSelection({});
    setSettings(defaultSettings);
    setTerminalShortcuts([]);
  }, []);

  return (
    <StorageContext.Provider
      value={{
        servers,
        recentRepos,
        recentFeatures,
        lastSelection,
        settings,
        terminalShortcuts,
        isLoading,
        addServer,
        updateServer,
        deleteServer,
        getServer,
        addRecentRepo,
        addRecentFeature,
        saveLastSelection,
        updateSettings,
        addTerminalShortcut,
        updateTerminalShortcut,
        deleteTerminalShortcut,
        clearAllData,
      }}>
      {children}
    </StorageContext.Provider>
  );
}

export function useStorage(): StorageContextType {
  const context = useContext(StorageContext);
  if (!context) {
    throw new Error('useStorage must be used within a StorageProvider');
  }
  return context;
}
