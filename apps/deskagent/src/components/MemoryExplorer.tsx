import { useMemo, useState } from "react";
import type { MemoryEvent } from "../lib/types.ts";
import { DEFAULT_FILTER, filterMemories, displayText, confidenceLabel, groupByKind } from "../lib/memory.ts";
import type { MemoryFilter } from "../lib/memory.ts";

export default function MemoryExplorer({ memories }: { memories: MemoryEvent[] }) {
  const [filter, setFilter] = useState<MemoryFilter>(DEFAULT_FILTER);
  const counts = useMemo(() => groupByKind(memories), [memories]);
  const filtered = useMemo(() => filterMemories(memories, filter), [memories, filter]);

  return (
    <section className="explorer">
      <header>
        <h2>Memory explorer</h2>
        <div className="counts muted small">
          {Object.entries(counts)
            .map(([k, v]) => `${k} ${v}`)
            .join(" · ")}
        </div>
      </header>
      <div className="filters">
        <select
          value={filter.kind}
          onChange={(e) => setFilter({ ...filter, kind: e.target.value as MemoryFilter["kind"] })}
        >
          <option value="all">All kinds</option>
          <option value="episodic">Episodic</option>
          <option value="semantic">Semantic</option>
          <option value="procedural">Procedural</option>
          <option value="working">Working</option>
        </select>
        <select
          value={filter.scope}
          onChange={(e) => setFilter({ ...filter, scope: e.target.value as MemoryFilter["scope"] })}
        >
          <option value="all">All scopes</option>
          <option value="companion">Companion</option>
          <option value="project">Project</option>
        </select>
        <select
          value={filter.approval}
          onChange={(e) => setFilter({ ...filter, approval: e.target.value as MemoryFilter["approval"] })}
        >
          <option value="all">All approvals</option>
          <option value="approved">Approved</option>
          <option value="pending">Pending</option>
          <option value="rejected">Rejected</option>
        </select>
        <input
          type="search"
          placeholder="Search memories…"
          value={filter.search}
          onChange={(e) => setFilter({ ...filter, search: e.target.value })}
        />
      </div>
      <ul className="memory-list">
        {filtered.length === 0 && <li className="muted">No memories match.</li>}
        {filtered.map((m) => (
          <li key={m.id} className={`memory ${m.approval}`}>
            <div className="memory-line">
              <span className="kind">{m.kind}</span>
              <span className={`conf ${confidenceLabel(m.confidence)}`}>{m.confidence.toFixed(2)}</span>
              <span className="scope">{m.scope.type}{m.scope.project_id ? `:${m.scope.project_id}` : ""}</span>
              <span className="approval">{m.approval}</span>
            </div>
            <div className="memory-text">{displayText(m)}</div>
            <div className="muted small">{m.created_at} · {m.source}</div>
          </li>
        ))}
      </ul>
    </section>
  );
}
