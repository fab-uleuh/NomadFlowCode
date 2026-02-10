import Link from 'next/link';
import { Terminal, GitBranch, Server } from 'lucide-react';
import { InstallCommand } from '@/components/install-command';

export default function HomePage() {
  return (
    <div className="flex flex-col items-center justify-center flex-1 px-4 py-16">
      <div className="mb-8">
        <svg
          width={80}
          height={80}
          viewBox="0 0 1024 1024"
          xmlns="http://www.w3.org/2000/svg"
        >
          <defs>
            <linearGradient id="heroGrad" x1="0%" y1="0%" x2="100%" y2="100%">
              <stop
                offset="0%"
                style={{ stopColor: '#5336E2', stopOpacity: 1 }}
              />
              <stop
                offset="100%"
                style={{ stopColor: '#8B5CF6', stopOpacity: 1 }}
              />
            </linearGradient>
          </defs>
          <rect width={1024} height={1024} rx={180} ry={180} fill="#0f0f17" />
          <path
            d="M256 768 Q512 256 768 768"
            stroke="url(#heroGrad)"
            strokeWidth={64}
            fill="none"
            strokeLinecap="round"
          />
        </svg>
      </div>

      <h1 className="text-4xl font-bold mb-4 text-center bg-gradient-to-r from-[#5336E2] to-[#8B5CF6] bg-clip-text text-transparent">
        NomadFlow
      </h1>
      <p className="text-lg text-fd-muted-foreground mb-10 text-center max-w-xl">
        Manage git worktrees and tmux sessions from your phone. A single Rust
        binary that runs on your server, paired with a mobile app for on-the-go
        development.
      </p>

      <div className="mb-10 w-full max-w-lg">
        <InstallCommand />
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-3xl w-full mb-10">
        <div className="border border-fd-border rounded-xl p-5 bg-fd-card transition-colors hover:border-[#5336E2]/40">
          <div className="mb-3 text-[#5336E2] dark:text-[#8B5CF6]">
            <Server size={24} />
          </div>
          <h3 className="font-semibold mb-2">Single Binary</h3>
          <p className="text-sm text-fd-muted-foreground">
            One Rust binary ships the HTTP server, WebSocket proxy, and TUI
            wizard. No Node.js, no Docker required.
          </p>
        </div>
        <div className="border border-fd-border rounded-xl p-5 bg-fd-card transition-colors hover:border-[#5336E2]/40">
          <div className="mb-3 text-[#5336E2] dark:text-[#8B5CF6]">
            <Terminal size={24} />
          </div>
          <h3 className="font-semibold mb-2">Mobile Terminal</h3>
          <p className="text-sm text-fd-muted-foreground">
            Full terminal access from your phone via ttyd. Browse repos, switch
            features, and code anywhere.
          </p>
        </div>
        <div className="border border-fd-border rounded-xl p-5 bg-fd-card transition-colors hover:border-[#5336E2]/40">
          <div className="mb-3 text-[#5336E2] dark:text-[#8B5CF6]">
            <GitBranch size={24} />
          </div>
          <h3 className="font-semibold mb-2">Git Worktrees</h3>
          <p className="text-sm text-fd-muted-foreground">
            Each feature branch gets its own worktree and tmux window. Switch
            context instantly, no stashing needed.
          </p>
        </div>
      </div>

      <div className="flex gap-4">
        <Link
          href="/docs"
          className="px-5 py-2.5 rounded-lg bg-gradient-to-r from-[#5336E2] to-[#8B5CF6] text-white font-medium hover:opacity-90 transition-opacity"
        >
          Get Started
        </Link>
        <a
          href="https://github.com/fab-uleuh/NomadFlowCode"
          className="px-5 py-2.5 rounded-lg border border-fd-border font-medium hover:bg-fd-accent transition-colors"
          target="_blank"
          rel="noopener noreferrer"
        >
          GitHub
        </a>
      </div>

      <footer className="mt-auto pt-16 pb-8 text-sm text-fd-muted-foreground flex gap-4">
        <Link href="/privacy" className="hover:underline">
          Privacy Policy
        </Link>
        <Link href="/terms" className="hover:underline">
          Terms of Service
        </Link>
        <Link href="/support" className="hover:underline">
          Support
        </Link>
      </footer>
    </div>
  );
}
