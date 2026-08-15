import { useState } from "react";
import type { ScheduledTask } from "../lib/tasks.ts";
import { createTask, isDue, markDone, roll } from "../lib/tasks.ts";

// Scheduled-tasks placeholder (Phase 6 Task 6): local list, no runner yet.
export default function TasksPanel() {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [name, setName] = useState("");
  const [cadence, setCadence] = useState<ScheduledTask["cadence"]>("daily");
  const now = new Date().toISOString();
  const due = tasks.filter((t) => isDue(t, now));

  function add() {
    if (!name.trim()) return;
    setTasks((prev) => [...prev, createTask(name.trim(), cadence)]);
    setName("");
  }

  return (
    <section className="tasks">
      <header>
        <h3>Scheduled tasks</h3>
        <span className="muted small">placeholder — no runner yet (Phase 6)</span>
      </header>
      {due.length > 0 && <p className="small amber">⚠ {due.length} task(s) due now</p>}
      <div className="filters">
        <input placeholder="task name" value={name} onChange={(e) => setName(e.target.value)} />
        <select value={cadence} onChange={(e) => setCadence(e.target.value as ScheduledTask["cadence"])}>
          <option value="once">once</option>
          <option value="daily">daily</option>
          <option value="weekly">weekly</option>
        </select>
        <button onClick={add}>Add</button>
      </div>
      <ul className="runs">
        {tasks.length === 0 && <li className="muted">No tasks scheduled.</li>}
        {tasks.map((t) => (
          <li key={t.id}>
            <span>{t.name} <span className="muted">({t.cadence} · {t.next_run.slice(0, 16).replace("T", " ")})</span></span>
            <span>
              {t.status === "scheduled" ? (
                <>
                  <button onClick={() => setTasks((prev) => prev.map((x) => (x.id === t.id ? roll(x, now) : x)))}>roll</button>
                  <button onClick={() => setTasks((prev) => prev.map((x) => (x.id === t.id ? markDone(x) : x)))}>done</button>
                </>
              ) : (
                <span className="muted">{t.status}</span>
              )}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}
