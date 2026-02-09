import React, { useState, useEffect } from 'react';
import { Box, Text } from 'ink';
import { Select, Spinner } from '@inkjs/ui';
import type { Server, Repository, Feature } from '../types.js';
import type { CliFeature } from '../lib/types.js';
import { listFeatures } from '../lib/api-client.js';
import { getPaneCommand, listWindows } from '../lib/tmux.js';

interface FeaturePickerProps {
  server: Server;
  repo: Repository;
  tmuxSession: string;
  onSelect: (feature: Feature) => void;
  onCreate: () => void;
  onBack: () => void;
}

export default function FeaturePicker({
  server,
  repo,
  tmuxSession,
  onSelect,
  onCreate,
  onBack,
}: FeaturePickerProps) {
  const [features, setFeatures] = useState<CliFeature[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();

  useEffect(() => {
    let cancelled = false;
    async function fetch() {
      const result = await listFeatures(server, repo.path);
      if (cancelled) return;

      if (result.success && result.data) {
        // Enrich features with local tmux info
        const tmuxWindows = listWindows(tmuxSession);
        const enriched = result.data.features.map(f => {
          const windowName = `${repo.name}:${f.name}`;
          const window = tmuxWindows.find(w => w.name === windowName);
          const paneCommand = window ? getPaneCommand(tmuxSession, windowName) : null;
          return { ...f, paneCommand: paneCommand ?? undefined };
        });
        setFeatures(enriched);
      } else {
        setError(result.error || 'Failed to fetch features');
      }
      setLoading(false);
    }
    fetch();
    return () => { cancelled = true; };
  }, [server, repo]);

  if (loading) {
    return (
      <Box>
        <Spinner label="Loading features..." />
      </Box>
    );
  }

  if (error) {
    return (
      <Box flexDirection="column">
        <Text color="red">Error: {error}</Text>
        <Text dimColor>Press Escape to go back</Text>
      </Box>
    );
  }

  const options = [
    ...features.map(f => {
      const isIdle = !f.paneCommand || ['bash', 'zsh', 'sh', 'fish'].includes(f.paneCommand.toLowerCase());
      const processInfo = f.paneCommand
        ? isIdle ? '  idle' : `  ● ${f.paneCommand} running`
        : '';
      const prefix = f.isMain ? '⌂ ' : '';
      const suffix = f.isMain ? '  [source]' : '';
      return {
        label: `${prefix}${f.name}  ${f.branch}${processInfo}${suffix}`,
        value: f.name,
      };
    }),
    { label: '+ Create a feature', value: '__create__' },
  ];

  return (
    <Box flexDirection="column">
      <Text bold>Select a feature ({repo.name}):</Text>
      <Select
        options={options}
        onChange={(value) => {
          if (value === '__create__') {
            onCreate();
          } else {
            const feature = features.find(f => f.name === value);
            if (feature) onSelect(feature);
          }
        }}
      />
    </Box>
  );
}
