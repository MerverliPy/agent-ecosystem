import type { Session } from "../lib/types.ts";
import { messageCount } from "../lib/sessions.ts";

export default function SessionList({
  sessions,
  activeId,
  onSelect,
}: {
  sessions: Session[];
  activeId: string | null;
  onSelect: (id: string) => void;
}) {
  if (sessions.length === 0) return <p className="muted small">No sessions yet.</p>;
  return (
    <ul className="session-list">
      {sessions.map((s) => (
        <li key={s.id}>
          <button className={s.id === activeId ? "session active" : "session"} onClick={() => onSelect(s.id)}>
            <span className="session-title">{s.title}</span>
            <span className="muted small">{messageCount(s)} msg</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
