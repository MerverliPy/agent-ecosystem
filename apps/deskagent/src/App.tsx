import { useEffect, useState } from "react";
import type { ApprovalCard, MemoryEvent, Persona, Session } from "./lib/types.ts";
import { getBridge } from "./lib/bridge.ts";
import { createSession, addMessage, sortSessions } from "./lib/sessions.ts";
import { pendingCards } from "./lib/approvals.ts";
import ChatWindow from "./components/ChatWindow.tsx";
import SessionList from "./components/SessionList.tsx";
import MemoryExplorer from "./components/MemoryExplorer.tsx";
import PersonaCard from "./components/PersonaCard.tsx";
import ApprovalCardView from "./components/ApprovalCard.tsx";

export default function App() {
  const bridge = getBridge();
  const [sessions, setSessions] = useState<Session[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [memories, setMemories] = useState<MemoryEvent[]>([]);
  const [approvals, setApprovals] = useState<ApprovalCard[]>([]);
  const [persona, setPersona] = useState<Persona | null>(null);
  const [tab, setTab] = useState<"chat" | "memory">("chat");
  const [status, setStatus] = useState("loading…");

  useEffect(() => {
    (async () => {
      try {
        const [s, m, a, p] = await Promise.all([
          bridge.listSessions(),
          bridge.listMemories(),
          bridge.listApprovals(),
          bridge.getPersona(),
        ]);
        setSessions(sortSessions(s));
        setMemories(m);
        setApprovals(a);
        setPersona(p);
        setStatus(s.length ? `ready · ${s.length} session(s)` : "ready");
        if (s.length > 0) setActiveId(s[0].id);
      } catch (err) {
        setStatus(`bridge error: ${err instanceof Error ? err.message : String(err)}`);
      }
    })();
  }, [bridge]);

  const active = sessions.find((s) => s.id === activeId) ?? null;

  async function handleNewSession() {
    const s = await bridge.createSession();
    setSessions((prev) => sortSessions([s, ...prev]));
    setActiveId(s.id);
    setTab("chat");
  }

  async function handleSend(content: string) {
    if (!active) return;
    const withUser = await bridge.appendMessage(active.id, "user", content);
    // Assistant echo — Phase 6 wires the local model runtime here.
    const withAssistant = await bridge.appendMessage(withUser.id, "assistant", `(model runtime lands in Phase 6)\n\n${content}`);
    setSessions((prev) => sortSessions(prev.map((s) => (s.id === withAssistant.id ? withAssistant : s))));
  }

  const pending = pendingCards(approvals);

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">DeskAgent</div>
        <div className="status" title={status}>{status}</div>
        <button className="primary" onClick={handleNewSession}>New session</button>
        <SessionList sessions={sessions} activeId={activeId} onSelect={setActiveId} />
        <nav className="tabs">
          <button className={tab === "chat" ? "tab active" : "tab"} onClick={() => setTab("chat")}>Chat</button>
          <button className={tab === "memory" ? "tab active" : "tab"} onClick={() => setTab("memory")}>Memory</button>
        </nav>
        {pending.length > 0 && (
          <div className="pending-badge">⚠ {pending.length} approval pending</div>
        )}
      </aside>
      <main className="content">
        {tab === "chat" ? (
          active ? (
            <ChatWindow session={active} onSend={handleSend} />
          ) : (
            <div className="empty">Pick a session or start a new one.</div>
          )
        ) : (
          <div className="memory-view">
            <MemoryExplorer memories={memories} />
            <div className="memory-side">
              <PersonaCard persona={persona} />
              <div className="approval-panel">
                <h3>Approvals</h3>
                {approvals.length === 0 && <p className="muted">No approval cards yet.</p>}
                {approvals.map((c) => (
                  <ApprovalCardView key={c.id} card={c} />
                ))}
              </div>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
