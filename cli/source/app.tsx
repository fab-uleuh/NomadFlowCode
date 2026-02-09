import React, { useState, useCallback, useEffect } from 'react';
import { Box, Text, useApp, useInput } from 'ink';
import { Spinner } from '@inkjs/ui';
import type { NomadFlowConfig } from './lib/config.js';
import { loadState, saveState } from './lib/state.js';
import { switchFeature } from './lib/api-client.js';
import { attachSession, sessionExists } from './lib/tmux.js';
import type { Server, Repository, Feature } from './types.js';
import type { CliState, WizardStep } from './lib/types.js';

import Header from './components/Header.js';
import Breadcrumb from './components/Breadcrumb.js';
import Resume from './screens/Resume.js';
import ServerPicker from './screens/ServerPicker.js';
import RepoPicker from './screens/RepoPicker.js';
import FeaturePicker from './screens/FeaturePicker.js';
import FeatureCreate from './screens/FeatureCreate.js';
import StatusView from './screens/StatusView.js';

interface AppProps {
  config: NomadFlowConfig;
  statusMode?: boolean;
  attachFeature?: string;
  repoFilter?: string;
}

export default function App({ config, statusMode, attachFeature, repoFilter }: AppProps) {
  const { exit } = useApp();

  // Status mode: just show and exit
  if (statusMode) {
    return (
      <Box flexDirection="column">
        <Header />
        <StatusView tmuxSession={config.tmux.session} />
      </Box>
    );
  }

  const cliState = loadState(config);
  const hasLastSession = !!(cliState.lastServer && cliState.lastRepo && cliState.lastFeature);

  const [step, setStep] = useState<WizardStep | 'create-feature' | 'attaching'>(
    hasLastSession && !attachFeature ? 'resume' : 'server',
  );
  const [server, setServer] = useState<Server | undefined>(
    // Pre-select if only one server or last used
    config.servers.length === 1 ? config.servers[0] : undefined,
  );
  const [repo, setRepo] = useState<Repository>();
  const [feature, setFeature] = useState<Feature>();
  const [attachError, setAttachError] = useState<string>();

  // Handle --attach shortcut
  useEffect(() => {
    if (attachFeature && server) {
      // If we have a repo filter or last repo, try to attach directly
      const repoPath = repoFilter || cliState.lastRepo;
      if (repoPath) {
        doAttach(server, repoPath, attachFeature);
      } else {
        setStep('repo');
      }
    }
  }, []);

  // Handle Escape to go back
  useInput((input, key) => {
    if (input === 'q') {
      exit();
      return;
    }
    if (key.escape) {
      goBack();
    }
  });

  function goBack() {
    switch (step) {
      case 'repo':
        setStep('server');
        setServer(undefined);
        break;
      case 'feature':
        setStep('repo');
        setRepo(undefined);
        break;
      case 'create-feature':
        setStep('feature');
        break;
      default:
        break;
    }
  }

  async function doAttach(srv: Server, repoPath: string, featureName: string) {
    setStep('attaching');

    // Call API to switch feature (prepares tmux window)
    const result = await switchFeature(srv, repoPath, featureName);

    if (!result.success) {
      setAttachError(result.error || 'Failed to switch feature');
      return;
    }

    // Save state for next time
    const newState: CliState = {
      lastServer: srv.id,
      lastRepo: repoPath,
      lastFeature: featureName,
      lastAttached: Date.now(),
    };
    saveState(config, newState);

    // Attach to tmux session
    if (sessionExists(config.tmux.session)) {
      exit();
      // Small delay to let Ink cleanup before we take over the terminal
      setTimeout(() => {
        attachSession(config.tmux.session);
      }, 100);
    } else {
      setAttachError('tmux session not found');
    }
  }

  // Auto-skip server selection if only one server
  const handleServerSelect = useCallback((srv: Server) => {
    setServer(srv);
    setStep('repo');
  }, []);

  const handleRepoSelect = useCallback((r: Repository) => {
    setRepo(r);
    if (attachFeature) {
      doAttach(server!, r.path, attachFeature);
    } else {
      setStep('feature');
    }
  }, [server, attachFeature]);

  const handleFeatureSelect = useCallback((f: Feature) => {
    setFeature(f);
    doAttach(server!, repo!.path, f.name);
  }, [server, repo]);

  const handleResume = useCallback(() => {
    const srv = config.servers.find(s => s.id === cliState.lastServer);
    if (srv && cliState.lastRepo && cliState.lastFeature) {
      setServer(srv);
      doAttach(srv, cliState.lastRepo, cliState.lastFeature);
    }
  }, [cliState]);

  const handleFeatureCreated = useCallback((featureName: string) => {
    doAttach(server!, repo!.path, featureName);
  }, [server, repo]);

  // Auto-skip to repo if only one server and no resume
  useEffect(() => {
    if (step === 'server' && config.servers.length === 1 && config.servers[0]) {
      handleServerSelect(config.servers[0]);
    }
  }, [step]);

  return (
    <Box flexDirection="column">
      <Header />
      <Breadcrumb server={server?.name} repo={repo?.name} feature={feature?.name} />

      {step === 'resume' && (
        <Resume
          state={cliState}
          onResume={handleResume}
          onSkip={() => setStep('server')}
        />
      )}

      {step === 'server' && config.servers.length > 1 && (
        <ServerPicker servers={config.servers} onSelect={handleServerSelect} />
      )}

      {step === 'repo' && server && (
        <RepoPicker
          server={server}
          lastRepo={cliState.lastRepo ? cliState.lastRepo.split('/').pop() : undefined}
          onSelect={handleRepoSelect}
          onBack={goBack}
        />
      )}

      {step === 'feature' && server && repo && (
        <FeaturePicker
          server={server}
          repo={repo}
          tmuxSession={config.tmux.session}
          onSelect={handleFeatureSelect}
          onCreate={() => setStep('create-feature')}
          onBack={goBack}
        />
      )}

      {step === 'create-feature' && server && repo && (
        <FeatureCreate
          server={server}
          repo={repo}
          onCreated={handleFeatureCreated}
          onBack={() => setStep('feature')}
        />
      )}

      {step === 'attaching' && !attachError && (
        <Box>
          <Spinner label="Preparing tmux session..." />
        </Box>
      )}

      {attachError && (
        <Box flexDirection="column">
          <Text color="red">Error: {attachError}</Text>
          <Text dimColor>Press q to quit</Text>
        </Box>
      )}

      <Box marginTop={1}>
        <Text dimColor>
          {step === 'attaching' ? '' : 'Escape: back  q: quit'}
        </Text>
      </Box>
    </Box>
  );
}
