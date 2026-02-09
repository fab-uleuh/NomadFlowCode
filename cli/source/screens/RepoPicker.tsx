import React, { useState, useEffect } from 'react';
import { Box, Text } from 'ink';
import { Select, Spinner } from '@inkjs/ui';
import type { Server, Repository } from '../types.js';
import { listRepos } from '../lib/api-client.js';

interface RepoPickerProps {
  server: Server;
  lastRepo?: string;
  onSelect: (repo: Repository) => void;
  onBack: () => void;
}

export default function RepoPicker({ server, lastRepo, onSelect, onBack }: RepoPickerProps) {
  const [repos, setRepos] = useState<Repository[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();

  useEffect(() => {
    let cancelled = false;
    async function fetch() {
      const result = await listRepos(server);
      if (cancelled) return;
      if (result.success && result.data) {
        setRepos(result.data.repos);
      } else {
        setError(result.error || 'Failed to fetch repos');
      }
      setLoading(false);
    }
    fetch();
    return () => { cancelled = true; };
  }, [server]);

  if (loading) {
    return (
      <Box>
        <Spinner label="Loading repos..." />
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

  if (repos.length === 0) {
    return (
      <Box flexDirection="column">
        <Text>No repositories found.</Text>
        <Text dimColor>Clone a repo via the mobile app or server first.</Text>
      </Box>
    );
  }

  // Sort: last used first
  const sorted = [...repos].sort((a, b) => {
    if (a.name === lastRepo) return -1;
    if (b.name === lastRepo) return 1;
    return 0;
  });

  const options = sorted.map(r => ({
    label: `${r.name}  ${r.branch}${r.name === lastRepo ? '  (last used)' : ''}`,
    value: r.path,
  }));

  return (
    <Box flexDirection="column">
      <Text bold>Select a repo ({server.name}):</Text>
      <Select
        options={options}
        onChange={(value) => {
          const repo = repos.find(r => r.path === value);
          if (repo) onSelect(repo);
        }}
      />
    </Box>
  );
}
