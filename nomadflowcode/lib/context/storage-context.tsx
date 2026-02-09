import AsyncStorage from '@react-native-async-storage/async-storage';
import React, { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';

import type { Server, Repository, Feature, AppSettings } from '../types';

/**
 * Migrate old server format (url) to new format (ttydUrl + apiUrl)
 * Also removes deprecated fields (url, isDefault)
 */
function migrateServer(server: any): Server {
  // Start with a clean server object
  const migrated: Server = {
    id: server.id,
    name: server.name,
    ttydUrl: server.ttydUrl,
    apiUrl: server.apiUrl,
    authToken: server.authToken,
    lastConnected: server.lastConnected,
  };

  // If already has ttydUrl, clean and return
  if (migrated.ttydUrl) {
    return migrated;
  }

  // Migrate from old 'url' field (WebSocket URL)
  const oldUrl = server.url || '';

  // Convert ws://host:7681/ws -> http://host:7681
  const ttydUrl = oldUrl
    .replace('wss://', 'https://')
    .replace('ws://', 'http://')
    .replace('/ws', '');

  migrated.ttydUrl = ttydUrl || 'http://localhost:7681';

  // Derive API URL: http://host:7681 -> http://host:8080
  if (!migrated.apiUrl && migrated.ttydUrl) {
    try {
      const url = new URL(migrated.ttydUrl);
      url.port = url.port === '7681' ? '8080' : url.port;
      migrated.apiUrl = url.toString().replace(/\/$/, '');
    } catch {
      migrated.apiUrl = migrated.ttydUrl.replace(':7681', ':8080');
    }
  }

  if (!migrated.apiUrl) {
    migrated.apiUrl = 'http://localhost:8080';
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
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    loadAllData();
  }, []);

  const loadAllData = async () => {
    try {
      const [serversData, recentReposData, recentFeaturesData, lastSelectionData, settingsData] =
        await Promise.all([
          AsyncStorage.getItem(STORAGE_KEYS.SERVERS),
          AsyncStorage.getItem(STORAGE_KEYS.RECENT_REPOS),
          AsyncStorage.getItem(STORAGE_KEYS.RECENT_FEATURES),
          AsyncStorage.getItem(STORAGE_KEYS.LAST_SELECTION),
          AsyncStorage.getItem(STORAGE_KEYS.SETTINGS),
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

  const clearAllData = useCallback(async (): Promise<void> => {
    await AsyncStorage.multiRemove(Object.values(STORAGE_KEYS));
    setServers([]);
    setRecentRepos([]);
    setRecentFeatures([]);
    setLastSelection({});
    setSettings(defaultSettings);
  }, []);

  return (
    <StorageContext.Provider
      value={{
        servers,
        recentRepos,
        recentFeatures,
        lastSelection,
        settings,
        isLoading,
        addServer,
        updateServer,
        deleteServer,
        getServer,
        addRecentRepo,
        addRecentFeature,
        saveLastSelection,
        updateSettings,
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
