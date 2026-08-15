import { loadSnapshot } from "../lib/skills";
import Browse from "../components/browse";

export default function Home() {
  const snap = loadSnapshot();
  return (
    <div className="container">
      <h1>SkillHub</h1>
      <p className="sub">
        npm for agent skills — install into Claude Code, Codex, Cursor, Gemini CLI, pi, OpenClaw, Copilot.
        Snapshot: {snap.updated_at}.
      </p>
      <Browse packages={snap.packages} />
      <p className="note" style={{ marginTop: 24 }}>
        Publish: <code>skillhub publish skillhub.json --files-dir .</code> · Install:{" "}
        <code>skillhub install owner/name</code>
      </p>
    </div>
  );
}
