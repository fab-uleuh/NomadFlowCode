import Link from 'next/link';

export default function HomePage() {
  return (
    <div className="flex flex-col items-center justify-center flex-1 px-4 py-16">
      <h1 className="text-4xl font-bold mb-4 text-center">NomadFlowCode</h1>
      <p className="text-lg text-fd-muted-foreground mb-8 text-center max-w-xl">
        Manage git worktrees and tmux sessions from your phone. A single Rust
        binary that runs on your server, paired with a mobile app for on-the-go
        development.
      </p>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-3xl w-full mb-10">
        <div className="border rounded-lg p-5">
          <h3 className="font-semibold mb-2">Single Binary</h3>
          <p className="text-sm text-fd-muted-foreground">
            One Rust binary ships the HTTP server, WebSocket proxy, and TUI
            wizard. No Node.js, no Docker required.
          </p>
        </div>
        <div className="border rounded-lg p-5">
          <h3 className="font-semibold mb-2">Mobile Terminal</h3>
          <p className="text-sm text-fd-muted-foreground">
            Full terminal access from your phone via ttyd. Browse repos, switch
            features, and code anywhere.
          </p>
        </div>
        <div className="border rounded-lg p-5">
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
          className="px-5 py-2.5 rounded-md bg-fd-primary text-fd-primary-foreground font-medium hover:opacity-90 transition-opacity"
        >
          Get Started
        </Link>
        <a
          href="https://github.com/fab-uleuh/NomadFlowCode"
          className="px-5 py-2.5 rounded-md border font-medium hover:bg-fd-accent transition-colors"
          target="_blank"
          rel="noopener noreferrer"
        >
          GitHub
        </a>
      </div>
    </div>
  );
}
