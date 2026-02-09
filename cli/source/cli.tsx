#!/usr/bin/env node
import React from 'react';
import { render } from 'ink';
import meow from 'meow';
import { loadConfig } from './lib/config.js';
import App from './app.js';

const cli = meow(
  `
  Usage
    $ nomadflow              Interactive wizard (default)

  Options
    --status                 Quick status: tmux session + windows + processes
    --attach <feature>       Attach directly to a feature
    --repo <name>            Skip repo selection (use with --attach)

  Navigation
    Up/Down                  Select
    Enter                    Confirm
    Escape                   Go back
    q                        Quit
`,
  {
    importMeta: import.meta,
    flags: {
      status: {
        type: 'boolean',
        default: false,
      },
      attach: {
        type: 'string',
      },
      repo: {
        type: 'string',
      },
    },
  },
);

const config = loadConfig();

render(
  <App
    config={config}
    statusMode={cli.flags.status}
    attachFeature={cli.flags.attach}
    repoFilter={cli.flags.repo}
  />,
);
