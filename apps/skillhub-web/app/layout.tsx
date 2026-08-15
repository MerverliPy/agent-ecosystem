import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "SkillHub — npm for agent skills",
  description: "Cross-harness skill registry: install agent skills into Claude Code, Codex, Cursor, Gemini CLI, pi, OpenClaw, Copilot.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
