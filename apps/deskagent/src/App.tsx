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
import ModelPicker from "./components/ModelPicker.tsx";
import TasksPanel from "./components/TasksPanel.tsx";

export default function App() {
  const bridge = getBridge();
  const [sessions, setSessions] = useState<Session[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [memories, setMemories] = useState<MemoryEvent[]>([]);
  const [approvals, setApprovals] = useState<ApprovalCard[]>([]);
  const [persona, setPersona] = useState<Persona | null>(null);
  const [tab, setTab] = useState<"chat" | "memory" | "models" | "tasks">("chat");
  const [status, setStatus] = useState("loading…");

  const refresh = async () => {
    try {
      const [m, a, p] = await Promise.all([
        bridge.listMemories(),
        bridge.listApprovals(),
        bridge.getPersona(),
      ]);
      setMemories(m);
      setApprovals(a);
      setPersona(p);
    } catch (err) {
      setStatus(`bridge error: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

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
    // Persist the user turn, then run the assistant turn through the (remembered)
    // local model via chat_complete (deterministic fallback when offline, DEC-0005).
    const withUser = await bridge.appendMessage(active.id, "user", content);
    let withAssistant: Session;
    try {
      withAssistant = await bridge.chatComplete(withUser.id, content);
    } catch (err) {
      withAssistant = await bridge.appendMessage(
        withUser.id,
        "assistant",
        `(runtime unavailable: ${err instanceof Error ? err.message : String(err)})`,
      );
    }
    setSessions((prev) => sortSessions(prev.map((s) => (s.id === withAssistant.id ? withAssistant : s))));
    // refresh memories/approvals/persona: chat may have enqueued new approval cards
    refresh();
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
          <button className={tab === "models" ? "tab active" : "tab"} onClick={() => setTab("models")}>Models</button>
          <button className={tab === "tasks" ? "tab active" : "tab"} onClick={() => setTab("tasks")}>Tasks</button>
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
        ) : tab === "models" ? (
          <div className="tab-page"><ModelPicker /></div>
        ) : tab === "tasks" ? (
          <div className="tab-page"><TasksPanel /></div>
        ) : (
          <div className="memory-view">
            <MemoryExplorer memories={memories} />
            <div className="memory-side">
              <PersonaCard persona={persona} />
              <div className="approval-panel">
                <h3>Approvals</h3>
                {approvals.length === 0 && <p className="muted">No approval cards yet.</p>}
                {approvals.map((c) => (
                  <ApprovalCardView
                    key={c.id}
                    card={c}
                    onDecide={async (cardId, approved) => {
                      await bridge.decideApproval(cardId, approved);
                      await refresh();
                    }}
                  />
                ))}
              </div>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
