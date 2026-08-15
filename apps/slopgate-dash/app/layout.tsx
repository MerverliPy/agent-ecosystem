import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "SlopGate — repo slop scores",
  description:
    "Per-repo AI-slop score history and trends, rendered from recorded check artifacts (slopgate-dash/data/history.json).",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
