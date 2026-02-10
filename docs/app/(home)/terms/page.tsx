import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Terms of Service - NomadFlowCode",
  description: "Terms of Service for the NomadFlowCode mobile application.",
};

export default function TermsPage() {
  return (
    <div className="max-w-3xl mx-auto px-4 py-16 prose prose-neutral dark:prose-invert">
      <h1>Terms of Service</h1>
      <p className="text-fd-muted-foreground">
        Last updated: February 10, 2026
      </p>

      <h2>1. Acceptance of Terms</h2>
      <p>
        By downloading, installing, or using NomadFlowCode (&quot;the
        app&quot;), you agree to be bound by these Terms of Service. If you do
        not agree to these terms, do not use the app.
      </p>

      <h2>2. Description of Service</h2>
      <p>
        NomadFlowCode is a mobile application that allows you to connect to your
        own self-hosted servers to manage git worktrees, tmux sessions, and
        access terminal sessions remotely. The app acts as a client and requires
        you to set up and maintain your own server infrastructure.
      </p>

      <h2>3. User Responsibilities</h2>
      <ul>
        <li>
          You are responsible for setting up, securing, and maintaining your own
          NomadFlowCode server.
        </li>
        <li>
          You are responsible for the security of your authentication tokens and
          server credentials.
        </li>
        <li>
          You are responsible for all activity that occurs through your server
          connections.
        </li>
        <li>
          You must comply with all applicable laws and regulations when using
          the app.
        </li>
      </ul>

      <h2>4. Intellectual Property</h2>
      <p>
        NomadFlowCode is open-source software. The source code is available
        under the terms specified in the project&apos;s{" "}
        <a
          href="https://github.com/fab-uleuh/NomadFlowCode"
          target="_blank"
          rel="noopener noreferrer"
        >
          GitHub repository
        </a>
        . Your use of the source code is governed by the applicable license.
      </p>

      <h2>5. Disclaimer of Warranties</h2>
      <p>
        The app is provided &quot;as is&quot; and &quot;as available&quot;
        without warranties of any kind, either express or implied, including but
        not limited to implied warranties of merchantability, fitness for a
        particular purpose, and non-infringement.
      </p>

      <h2>6. Limitation of Liability</h2>
      <p>
        To the fullest extent permitted by applicable law, NomadFlowCode and its
        contributors shall not be liable for any indirect, incidental, special,
        consequential, or punitive damages, or any loss of profits or revenues,
        whether incurred directly or indirectly, or any loss of data, use,
        goodwill, or other intangible losses resulting from:
      </p>
      <ul>
        <li>Your use or inability to use the app.</li>
        <li>
          Any unauthorized access to or alteration of your server or data.
        </li>
        <li>Any third-party conduct related to the service.</li>
      </ul>

      <h2>7. Termination</h2>
      <p>
        You may stop using the app at any time by uninstalling it from your
        device. These terms remain in effect as long as you use the app.
      </p>

      <h2>8. Changes to Terms</h2>
      <p>
        We reserve the right to modify these terms at any time. Changes will be
        posted on this page with an updated revision date. Continued use of the
        app after changes constitutes acceptance of the new terms.
      </p>

      <h2>9. Contact</h2>
      <p>
        For questions about these terms, contact us at{" "}
        <a href="mailto:fab.uleuh@gmail.com">fab.uleuh@gmail.com</a> or open an
        issue on our{" "}
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
