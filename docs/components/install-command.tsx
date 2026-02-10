'use client';

import { useState } from 'react';
import { Check, Copy } from 'lucide-react';

const COMMAND =
  "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/fab-uleuh/NomadFlowCode/releases/latest/download/nomadflow-installer.sh | sh";

export function InstallCommand() {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(COMMAND);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <button
      onClick={copy}
      className="group flex items-center gap-3 px-5 py-3 rounded-lg bg-fd-card border border-fd-border font-mono text-sm hover:border-[#5336E2]/40 transition-colors w-full max-w-lg cursor-pointer"
    >
      <span className="text-[#5336E2] dark:text-[#8B5CF6] select-none">$</span>
      <span className="flex-1 text-left truncate">{COMMAND}</span>
      {copied ? (
        <Check size={16} className="text-green-500 shrink-0" />
      ) : (
        <Copy
          size={16}
          className="text-fd-muted-foreground group-hover:text-fd-foreground shrink-0 transition-colors"
        />
      )}
    </button>
  );
}
