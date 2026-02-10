import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = {
  title: "Support - NomadFlowCode",
  description: "Get help and support for the NomadFlowCode mobile application.",
};

export default function SupportPage() {
  return (
    <div className="max-w-3xl mx-auto px-4 py-16 prose prose-neutral dark:prose-invert">
      <h1>Support</h1>
      <p>
        Need help with NomadFlowCode? Here are the best ways to get assistance.
      </p>

      <h2>Documentation</h2>
      <p>
        Our <Link href="/docs">documentation</Link> covers installation, setup,
        configuration, and usage of both the server and the mobile app.
      </p>

      <h2>Report a Bug or Request a Feature</h2>
      <p>
        If you encounter a bug or have a feature request, please open an issue
        on our GitHub repository:
      </p>
      <p>
        <a
          href="https://github.com/fab-uleuh/NomadFlowCode/issues"
          target="_blank"
          rel="noopener noreferrer"
        >
          github.com/fab-uleuh/NomadFlowCode/issues
        </a>
      </p>

      <h2>Contact Us</h2>
      <p>
        For general questions or support inquiries, you can reach us by email:
      </p>
      <p>
        <a href="mailto:fab.uleuh@gmail.com">fab.uleuh@gmail.com</a>
      </p>

      <h2>Community</h2>
      <p>
        NomadFlowCode is an open-source project. You can contribute, ask
        questions, and participate in discussions on{" "}
        <a
          href="https://github.com/fab-uleuh/NomadFlowCode"
          target="_blank"
          rel="noopener noreferrer"
        >
          GitHub
        </a>
        .
      </p>
    </div>
  );
}
