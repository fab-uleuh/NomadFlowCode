import Link from 'next/link';
import { Terminal, GitBranch, Server } from 'lucide-react';
import { InstallCommand } from '@/components/install-command';
import { TerminalReplay } from '@/components/terminal-replay';

export default function HomePage() {
  return (
    <div className="flex flex-col items-center flex-1 px-4 sm:px-6 py-10 lg:py-16">
      {/* Hero — centered text */}
      <div className="flex flex-col items-center max-w-2xl w-full mb-10 lg:mb-12">
        <div className="mb-4 lg:mb-6">
          <svg
            width={64}
            height={64}
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
        <p className="text-base lg:text-lg text-fd-muted-foreground mb-6 lg:mb-8 text-center">
          Manage git worktrees and PTY sessions from your phone. A single Rust
          binary that runs on your server, paired with a mobile app for on-the-go
          development.
        </p>

        <div className="mb-6 lg:mb-8 w-full overflow-hidden flex justify-center">
          <InstallCommand />
        </div>

        <div className="flex flex-wrap justify-center gap-3 sm:gap-4">
          <Link
            href="/docs"
            className="px-5 py-2.5 sm:px-6 sm:py-3 rounded-lg bg-gradient-to-r from-[#5336E2] to-[#8B5CF6] text-white font-medium text-base sm:text-lg hover:opacity-90 transition-opacity"
          >
            Get Started
          </Link>
          <a
            href="https://github.com/fab-uleuh/NomadFlowCode"
            className="px-5 py-2.5 sm:px-6 sm:py-3 rounded-lg border border-fd-border font-medium text-base sm:text-lg hover:bg-fd-accent transition-colors"
            target="_blank"
            rel="noopener noreferrer"
          >
            GitHub
          </a>
          <a
            href="https://apps.apple.com/fr/app/nomadflowcode/id6758987619"
            className="inline-flex items-center gap-2 px-5 py-2.5 sm:px-6 sm:py-3 rounded-lg border border-fd-border font-medium text-base sm:text-lg hover:bg-fd-accent transition-colors"
            target="_blank"
            rel="noopener noreferrer"
          >
            <svg width="18" height="18" viewBox="0 0 384 512" fill="currentColor">
              <path d="M318.7 268.7c-.2-36.7 16.4-64.4 50-84.8-18.8-26.9-47.2-41.7-84.7-44.6-35.5-2.8-74.3 20.7-88.5 20.7-15 0-49.4-19.7-76.4-19.7C63.3 141.2 4 184.8 4 273.5q0 39.3 14.4 81.2c12.8 36.7 59 126.7 107.2 125.2 25.2-.6 43-17.9 75.8-17.9 31.8 0 48.3 17.9 76.4 17.9 48.6-.7 90.4-82.5 102.6-119.3-65.2-30.7-61.7-90-61.7-91.9zm-56.6-164.2c27.3-32.4 24.8-61.9 24-72.5-24.1 1.4-52 16.4-67.9 34.9-17.5 19.8-27.8 44.3-25.6 71.9 26.1 2 49.9-11.4 69.5-34.3z" />
            </svg>
            App Store
          </a>
        </div>
      </div>

      {/* Demo showcase — terminal + phone side by side */}
      <div className="w-full max-w-4xl mb-12 lg:mb-16">
        <p className="text-sm text-fd-muted-foreground text-center mb-4 lg:mb-6">
          3 commands. That&apos;s it.
        </p>
        <div className="flex flex-col lg:flex-row items-center gap-6 lg:gap-8 justify-center">
          {/* Terminal replay */}
          <div className="w-full max-w-lg">
            <TerminalReplay />
          </div>

          {/* Phone mockup */}
          <div className="flex-shrink-0">
            <div className="relative mx-auto w-[180px] sm:w-[200px] lg:w-[240px] rounded-[2rem] lg:rounded-[2.5rem] border-[5px] lg:border-[6px] border-fd-foreground/20 bg-black p-1.5 lg:p-2 shadow-2xl shadow-[#5336E2]/15">
              {/* Notch */}
              <div className="absolute top-0 left-1/2 -translate-x-1/2 w-16 sm:w-20 lg:w-24 h-4 lg:h-5 bg-black rounded-b-xl lg:rounded-b-2xl z-10" />
              {/* Screen */}
              <div className="rounded-[1.5rem] lg:rounded-[2rem] overflow-hidden bg-black">
                <video
                  autoPlay
                  loop
                  muted
                  playsInline
                  className="w-full h-auto"
                >
                  <source src="/demo.mp4" type="video/mp4" />
                </video>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Features */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-3xl w-full mb-10">
        <div className="border border-fd-border rounded-xl p-5 bg-fd-card transition-colors hover:border-[#5336E2]/40">
          <div className="mb-3 text-[#5336E2] dark:text-[#8B5CF6]">
            <Server size={24} />
          </div>
          <h3 className="font-semibold mb-2">Single Binary</h3>
          <p className="text-sm text-fd-muted-foreground">
            One Rust binary ships the HTTP server, PTY multiplexer, and TUI
            wizard. No Node.js, no Docker required.
          </p>
        </div>
        <div className="border border-fd-border rounded-xl p-5 bg-fd-card transition-colors hover:border-[#5336E2]/40">
          <div className="mb-3 text-[#5336E2] dark:text-[#8B5CF6]">
            <Terminal size={24} />
          </div>
          <h3 className="font-semibold mb-2">Mobile Terminal</h3>
          <p className="text-sm text-fd-muted-foreground">
            Full terminal access from your phone via native PTY. Browse repos,
            switch features, and code anywhere.
          </p>
        </div>
        <div className="border border-fd-border rounded-xl p-5 bg-fd-card transition-colors hover:border-[#5336E2]/40">
          <div className="mb-3 text-[#5336E2] dark:text-[#8B5CF6]">
            <GitBranch size={24} />
          </div>
          <h3 className="font-semibold mb-2">Git Worktrees</h3>
          <p className="text-sm text-fd-muted-foreground">
            Each feature branch gets its own worktree and dedicated PTY pane.
            Switch context instantly, no stashing needed.
          </p>
        </div>
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
