import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Privacy Policy - NomadFlowCode",
  description: "Privacy Policy for the NomadFlowCode mobile application.",
};

export default function PrivacyPage() {
  return (
    <div className="max-w-3xl mx-auto px-4 py-16 prose prose-neutral dark:prose-invert">
      <h1>Privacy Policy</h1>
      <p className="text-fd-muted-foreground">
        Last updated: February 10, 2026
      </p>

      <h2>Introduction</h2>
      <p>
        NomadFlowCode (&quot;we&quot;, &quot;our&quot;, or &quot;the app&quot;)
        is a mobile application that connects to your own self-hosted server to
        manage git worktrees, tmux sessions, and terminal access. This privacy
        policy explains how we handle your data.
      </p>

      <h2>Data We Collect</h2>
      <p>
        NomadFlowCode is designed with privacy in mind. The app operates as a
        client that connects to servers you configure and control.
      </p>

      <h3>Data Stored Locally on Your Device</h3>
      <ul>
        <li>
          <strong>Server configurations</strong>: server names, URLs, and
          authentication tokens you provide to connect to your servers.
        </li>
        <li>
          <strong>App preferences</strong>: your display and usage preferences.
        </li>
      </ul>

      <h3>Data We Do Not Collect</h3>
      <ul>
        <li>
          We do not collect personal information (name, email, phone number).
        </li>
        <li>We do not collect usage analytics or telemetry.</li>
        <li>We do not track your location.</li>
        <li>We do not use advertising identifiers.</li>
        <li>We do not share any data with third parties.</li>
      </ul>

      <h2>Network Communication</h2>
      <p>
        The app communicates exclusively with servers that you configure. All
        network traffic goes directly between your device and your self-hosted
        NomadFlowCode server. We do not operate any intermediary servers and do
        not intercept or relay your data.
      </p>

      <h2>Data Storage and Security</h2>
      <p>
        All configuration data, including server URLs and authentication tokens,
        is stored locally on your device using the platform&apos;s secure
        storage mechanisms. Authentication tokens are transmitted over the
        network only when connecting to your configured servers.
      </p>

      <h2>Third-Party Services</h2>
      <p>
        NomadFlowCode does not integrate any third-party analytics, advertising,
        or tracking services. The app does not send data to any service other
        than the servers you explicitly configure.
      </p>

      <h2>Children&apos;s Privacy</h2>
      <p>
        NomadFlowCode is not directed at children under 13. We do not knowingly
        collect any personal information from children.
      </p>

      <h2>Changes to This Policy</h2>
      <p>
        We may update this privacy policy from time to time. Any changes will be
        posted on this page with an updated revision date.
      </p>

      <h2>Contact Us</h2>
      <p>
        If you have any questions about this privacy policy, please contact us
        at <a href="mailto:fab.uleuh@gmail.com">fab.uleuh@gmail.com</a> or open
        an issue on our{" "}
        <a
          href="https://github.com/fab-uleuh/NomadFlowCode/issues"
          target="_blank"
          rel="noopener noreferrer"
        >
          GitHub repository
        </a>
        .
      </p>
    </div>
  );
}
