import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "BenchKit — can my machine run it?",
  description:
    "Local-inference benchmark matrix: models × hardware × runtime × quantization. Every row links to an attributable source (DEC-0006).",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
