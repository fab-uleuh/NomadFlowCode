import React, { useState } from 'react';
import { Box, Text } from 'ink';
import { TextInput, Spinner, ConfirmInput } from '@inkjs/ui';
import type { Server, Repository } from '../lib/types.js';
import { createFeature } from '../lib/api-client.js';

interface FeatureCreateProps {
  server: Server;
  repo: Repository;
  onCreated: (featureName: string) => void;
  onBack: () => void;
}

type Step = 'name' | 'confirm' | 'creating' | 'done' | 'error';

export default function FeatureCreate({ server, repo, onCreated, onBack }: FeatureCreateProps) {
  const [step, setStep] = useState<Step>('name');
  const [featureName, setFeatureName] = useState('');
  const [error, setError] = useState<string>();

  async function doCreate() {
    setStep('creating');
    const result = await createFeature(server, repo.path, featureName);
    if (result.success) {
      setStep('done');
      onCreated(featureName);
    } else {
      setError(result.error || 'Failed to create feature');
      setStep('error');
    }
  }

  if (step === 'name') {
    return (
      <Box flexDirection="column">
        <Text bold>Create a new feature ({repo.name}):</Text>
        <Box>
          <Text>Feature name: </Text>
          <TextInput
            placeholder="my-feature"
            onSubmit={(value) => {
              if (value.trim()) {
                setFeatureName(value.trim());
                setStep('confirm');
              }
            }}
          />
        </Box>
      </Box>
    );
  }

  if (step === 'confirm') {
    return (
      <Box flexDirection="column">
        <Text>
          Create feature <Text bold color="cyan">{featureName}</Text> in{' '}
          <Text bold>{repo.name}</Text>?
        </Text>
        <ConfirmInput
          onConfirm={() => { doCreate(); }}
          onCancel={() => setStep('name')}
        />
      </Box>
    );
  }

  if (step === 'creating') {
    return (
      <Box>
        <Spinner label={`Creating feature ${featureName}...`} />
      </Box>
    );
  }

  if (step === 'error') {
    return (
      <Box flexDirection="column">
        <Text color="red">Error: {error}</Text>
        <Text dimColor>Press Escape to go back</Text>
      </Box>
    );
  }

  // step === 'done' - handled by onCreated callback
  return null;
}
