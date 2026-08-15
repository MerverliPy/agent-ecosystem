import { useState } from "react";
import type { Session } from "../lib/types.ts";
import { citationLine } from "../lib/memory.ts";

export default function ChatWindow({ session, onSend }: { session: Session; onSend: (content: string) => void }) {
  const [draft, setDraft] = useState("");
  const canSend = draft.trim().length > 0;

  function submit() {
    if (!canSend) return;
    onSend(draft.trim());
    setDraft("");
  }

  return (
    <section className="chat">
      <header className="chat-header">
        <h2>{session.title}</h2>
        <span className="muted">{session.messages.length} message(s)</span>
      </header>
      <div className="messages">
        {session.messages.length === 0 && (
          <p className="empty">Say something — DeskAgent will remember it (with your approval).</p>
        )}
        {session.messages.map((m) => (
          <div key={m.id} className={`msg ${m.role}`}>
            <div className="bubble">{m.content}</div>
            {m.citations && m.citations.length > 0 && (
              <div className="citations">
                <span className="muted">I remember…</span>
                {m.citations.map((c) => (
                  <div key={c.id} className="citation">{citationLine(c)}</div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
      <footer className="composer">
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
          }}
          placeholder="Message DeskAgent… (Enter to send)"
          rows={3}
        />
        <button className="primary" onClick={submit} disabled={!canSend}>Send</button>
      </footer>
    </section>
  );
}
