import React from 'react';
import { Box, Text } from 'ink';

interface BreadcrumbProps {
  server?: string;
  repo?: string;
  feature?: string;
}

export default function Breadcrumb({ server, repo, feature }: BreadcrumbProps) {
  const parts: string[] = [];
  if (server) parts.push(server);
  if (repo) parts.push(repo);
  if (feature) parts.push(feature);

  if (parts.length === 0) return null;

  return (
    <Box marginBottom={1}>
      {parts.map((part, i) => (
        <React.Fragment key={i}>
          {i > 0 && <Text dimColor> &gt; </Text>}
          <Text color={i === parts.length - 1 ? 'yellow' : 'white'}>{part}</Text>
        </React.Fragment>
      ))}
    </Box>
  );
}
