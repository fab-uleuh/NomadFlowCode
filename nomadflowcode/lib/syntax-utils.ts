const EXTENSION_MAP: Record<string, string> = {
  // Web
  ts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  jsx: 'jsx',
  json: 'json',
  html: 'markup',
  css: 'css',
  scss: 'scss',
  less: 'less',
  // Systems
  rs: 'rust',
  go: 'go',
  c: 'c',
  cpp: 'cpp',
  h: 'c',
  hpp: 'cpp',
  // Scripting
  py: 'python',
  rb: 'ruby',
  sh: 'bash',
  bash: 'bash',
  zsh: 'bash',
  fish: 'bash',
  // Config/Data
  yaml: 'yaml',
  yml: 'yaml',
  toml: 'toml',
  xml: 'markup',
  md: 'markdown',
  mdx: 'markdown',
  graphql: 'graphql',
  gql: 'graphql',
  sql: 'sql',
  // Other
  java: 'java',
  kt: 'kotlin',
  swift: 'swift',
  dart: 'dart',
  lua: 'lua',
  php: 'php',
};

const FILENAME_MAP: Record<string, string> = {
  'Dockerfile': 'docker',
  'dockerfile': 'docker',
  'Makefile': 'makefile',
  'makefile': 'makefile',
  'Cargo.toml': 'toml',
  'Cargo.lock': 'toml',
};

/**
 * Detect the syntax highlighting language from a file path.
 * Returns the Prism language identifier, or 'text' for unknown extensions.
 */
export function getLanguageFromPath(filePath: string): string {
  const fileName = filePath.split('/').pop() || '';

  // Check exact filename matches (e.g. Dockerfile, Makefile, Cargo.toml)
  if (FILENAME_MAP[fileName]) return FILENAME_MAP[fileName];

  // Check extension
  const ext = fileName.split('.').pop()?.toLowerCase();
  if (ext && EXTENSION_MAP[ext]) return EXTENSION_MAP[ext];

  return 'text';
}
