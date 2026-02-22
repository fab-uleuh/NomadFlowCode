'use client';

import { useState, useEffect, useCallback, useRef } from 'react';

/*
 * The animation has two phases:
 *   1. Setup wizard (simulated TUI screens that replace each other)
 *   2. Shell commands (link + serve with typed commands and output)
 */

// ── Phase 1: Wizard screens ────────────────────────────────────────────

interface WizardScreen {
  lines: string[];
  /** ms to hold this screen before moving to next */
  hold: number;
}

const WIZARD_SCREENS: WizardScreen[] = [
  {
    lines: [
      '\x1b[cyan]Welcome to NomadFlow!\x1b[/cyan]',
      '',
      '\x1b[bold]Set a password to secure your server:\x1b[/bold]',
      '',
      '\x1b[cyan]> Generate a password (recommended)\x1b[/cyan]',
      '\x1b[dim]  Enter my own password\x1b[/dim]',
    ],
    hold: 1500,
  },
  {
    lines: [
      '\x1b[bold]Will you use public tunnel mode? (y/n)\x1b[/bold]',
      '',
      '\x1b[dim]This exposes your server over the internet via a tunnel URL.\x1b[/dim]',
    ],
    hold: 1200,
  },
  {
    lines: [
      '\x1b[bold]Use a fixed subdomain for a stable public URL? (y/n)\x1b[/bold]',
      '',
      '  Your subdomain: \x1b[cyan]nf-fabien\x1b[/cyan]',
      '\x1b[dim]  -> https://nf-fabien.tunnel.nomadflowcode.dev\x1b[/dim]',
      '',
      '\x1b[dim]y: use this fixed subdomain  n: random URL each time\x1b[/dim]',
    ],
    hold: 1500,
  },
  {
    lines: [
      '\x1b[cyan]Configuration summary:\x1b[/cyan]',
      '',
      '  Password:    \x1b[yellow]k7Fp2xR···mQ9\x1b[/yellow]',
      '  Public mode: \x1b[bold]yes\x1b[/bold]',
      '  Subdomain:   \x1b[bold]nf-fabien\x1b[/bold]',
      '\x1b[dim]  Config: ~/.nomadflowcode/config.toml\x1b[/dim]',
      '',
      '\x1b[bold]Save and continue? (y/n)\x1b[/bold]',
    ],
    hold: 2000,
  },
];

// ── Phase 2: Shell commands ─────────────────────────────────────────────

interface ShellStep {
  command: string;
  output: string[];
  delay: number;
}

const SHELL_STEPS: ShellStep[] = [
  {
    command: 'nomadflow link ~/my-project',
    output: ['\x1b[dim]Linked my-project -> /Users/me/my-project\x1b[/dim]'],
    delay: 800,
  },
  {
    command: 'nomadflow serve',
    output: [
      '',
      // All content lines: exactly 38 visible chars between ║ and ║
      '  \x1b[box]╔══════════════════════════════════════╗\x1b[/box]',
      '  \x1b[box]║\x1b[/box]        \x1b[bold]NomadFlow Server Ready\x1b[/bold]        \x1b[box]║\x1b[/box]',
      '  \x1b[box]╠══════════════════════════════════════╣\x1b[/box]',
      '  \x1b[box]║\x1b[/box]                                      \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]        \x1b[qr]█▀▀▀▀▀█ ██ ██ █▀▀▀▀▀█\x1b[/qr]         \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]        \x1b[qr]█ ███ █  █▀██ █ ███ █\x1b[/qr]         \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]        \x1b[qr]█ ███ █ █▄ █▄ █ ███ █\x1b[/qr]         \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]        \x1b[qr]▀▀▀▀▀▀▀ █ █ █ ▀▀▀▀▀▀▀\x1b[/qr]         \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]        \x1b[qr]██▀█▄ ▀██▄▀██▀▄▀█▄██▀\x1b[/qr]         \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]        \x1b[qr]█▀▀▀▀▀█ ▄█▀██ █ █ ▀▄▀\x1b[/qr]         \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]        \x1b[qr]█ ███ █ █▄▀▄█▀▄▀███▀▀\x1b[/qr]         \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]        \x1b[qr]▀▀▀▀▀▀▀ ▀  ▀ ▀▀▀ ▀ ▀▀\x1b[/qr]         \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]                                      \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]    \x1b[dim]Scan this QR code from the app\x1b[/dim]    \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]    \x1b[dim]or enter manually:\x1b[/dim]                \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]                                      \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]    \x1b[dim]URL    :\x1b[/dim] \x1b[url]http://192.168.1.42:8080\x1b[/url] \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]    \x1b[dim]Secret :\x1b[/dim] \x1b[yellow]k7Fp2xR···mQ9\x1b[/yellow]            \x1b[box]║\x1b[/box]',
      '  \x1b[box]║\x1b[/box]                                      \x1b[box]║\x1b[/box]',
      '  \x1b[box]╚══════════════════════════════════════╝\x1b[/box]',
    ],
    delay: 800,
  },
];

const TYPE_SPEED = 45;
const OUTPUT_LINE_DELAY = 50;
const RESTART_DELAY = 5000;

// ── Tag renderer ────────────────────────────────────────────────────────

const CLASS_MAP: Record<string, string> = {
  green: 'text-emerald-400',
  bold: 'text-white font-semibold',
  dim: 'text-zinc-500',
  box: 'text-[#5336E2]',
  qr: 'text-white',
  url: 'text-emerald-400',
  yellow: 'text-amber-400',
  cyan: 'text-cyan-400',
};

function renderLine(line: string) {
  const parts: { text: string; className: string }[] = [];
  let remaining = line;

  while (remaining.length > 0) {
    const tagMatch = remaining.match(/\x1b\[(\w+)](.*?)\x1b\[\/\1]/);
    if (tagMatch && tagMatch.index !== undefined) {
      if (tagMatch.index > 0) {
        parts.push({ text: remaining.slice(0, tagMatch.index), className: '' });
      }
      parts.push({
        text: tagMatch[2],
        className: CLASS_MAP[tagMatch[1]] || '',
      });
      remaining = remaining.slice(tagMatch.index + tagMatch[0].length);
    } else {
      parts.push({ text: remaining, className: '' });
      break;
    }
  }

  return parts.map((part, i) => (
    <span key={i} className={part.className}>
      {part.text}
    </span>
  ));
}

// ── Component ───────────────────────────────────────────────────────────

type Phase = 'wizard' | 'shell';

export function TerminalReplay() {
  const [phase, setPhase] = useState<Phase>('wizard');
  const [wizardLines, setWizardLines] = useState<string[]>([]);
  const [shellLines, setShellLines] = useState<{ type: 'prompt' | 'output'; text: string }[]>([]);
  const [currentTyping, setCurrentTyping] = useState('');
  const [showCursor, setShowCursor] = useState(true);
  const [isTyping, setIsTyping] = useState(false);
  const cancelledRef = useRef(false);

  const runAnimation = useCallback(async () => {
    cancelledRef.current = false;

    // ── Phase 1: Wizard ──
    setPhase('wizard');
    setShellLines([]);
    setCurrentTyping('');
    setIsTyping(false);

    for (const screen of WIZARD_SCREENS) {
      if (cancelledRef.current) return;
      setWizardLines(screen.lines);
      await sleep(screen.hold);
    }

    // Brief pause before switching to shell
    await sleep(400);
    if (cancelledRef.current) return;

    // ── Phase 2: Shell commands ──
    setPhase('shell');
    setWizardLines([]);
    setIsTyping(true);

    for (const step of SHELL_STEPS) {
      if (cancelledRef.current) return;
      await sleep(step.delay);
      if (cancelledRef.current) return;

      if (step.command) {
        for (let i = 0; i <= step.command.length; i++) {
          if (cancelledRef.current) return;
          setCurrentTyping(step.command.slice(0, i));
          await sleep(TYPE_SPEED);
        }
        await sleep(250);
        if (cancelledRef.current) return;
        setShellLines((prev) => [...prev, { type: 'prompt', text: step.command }]);
        setCurrentTyping('');
      }

      for (const line of step.output) {
        if (cancelledRef.current) return;
        await sleep(OUTPUT_LINE_DELAY);
        setShellLines((prev) => [...prev, { type: 'output', text: line }]);
      }
    }

    setIsTyping(false);
    await sleep(RESTART_DELAY);
    if (!cancelledRef.current) runAnimation();
  }, []);

  useEffect(() => {
    runAnimation();
    return () => { cancelledRef.current = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const id = setInterval(() => setShowCursor((v) => !v), 530);
    return () => clearInterval(id);
  }, []);

  return (
    <div className="w-full rounded-xl border border-fd-border bg-[#0a0a12] shadow-2xl shadow-[#5336E2]/10 overflow-hidden">
      {/* Title bar */}
      <div className="flex items-center gap-2 px-4 py-2.5 bg-[#111119] border-b border-white/5">
        <div className="flex gap-1.5">
          <div className="w-3 h-3 rounded-full bg-[#ff5f57]" />
          <div className="w-3 h-3 rounded-full bg-[#febc2e]" />
          <div className="w-3 h-3 rounded-full bg-[#28c840]" />
        </div>
        <span className="text-xs text-zinc-500 ml-2 font-mono">
          {phase === 'wizard' ? 'nomadflow — setup' : 'terminal'}
        </span>
      </div>

      {/* Terminal body — responsive fixed height */}
      <div className="p-3 sm:p-4 font-mono text-[11px] sm:text-[12px] leading-[1.7] text-zinc-300 overflow-hidden h-[320px] sm:h-[400px] lg:h-[460px]">
        {phase === 'wizard' && (
          <>
            {wizardLines.map((line, i) => (
              <div key={i} className="whitespace-pre">
                {renderLine(line)}
              </div>
            ))}
            <span
              className={`inline-block w-[7px] h-[14px] ml-[1px] align-middle ${
                showCursor ? 'bg-zinc-300' : 'bg-transparent'
              }`}
            />
          </>
        )}

        {phase === 'shell' && (
          <>
            {shellLines.map((line, i) => (
              <div key={i} className="whitespace-pre">
                {line.type === 'prompt' ? (
                  <>
                    <span className="text-emerald-400">$</span>{' '}
                    <span className="text-white">{line.text}</span>
                  </>
                ) : (
                  renderLine(line.text)
                )}
              </div>
            ))}
            {isTyping && (
              <div className="whitespace-pre">
                <span className="text-emerald-400">$</span>{' '}
                <span className="text-white">{currentTyping}</span>
                <span
                  className={`inline-block w-[7px] h-[14px] ml-[1px] align-middle ${
                    showCursor ? 'bg-zinc-300' : 'bg-transparent'
                  }`}
                />
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
