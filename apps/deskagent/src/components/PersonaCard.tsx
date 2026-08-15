import type { Persona } from "../lib/types.ts";

export default function PersonaCard({ persona }: { persona: Persona | null }) {
  if (!persona) {
    return (
      <section className="persona">
        <h3>Persona</h3>
        <p className="muted small">Not generated yet — the consolidation pass builds it after ~50 new memories.</p>
      </section>
    );
  }
  return (
    <section className="persona">
      <h3>Persona v{persona.version}</h3>
      <p className="small">{persona.summary}</p>
      <details>
        <summary>Facts ({persona.facts.length})</summary>
        <ul className="small">
          {persona.facts.map((f, i) => (
            <li key={i}>{f}</li>
          ))}
        </ul>
      </details>
      <details>
        <summary>Preferences ({persona.preferences.length})</summary>
        <ul className="small">
          {persona.preferences.map((p, i) => (
            <li key={i}>{p}</li>
          ))}
        </ul>
      </details>
      <details>
        <summary>Skills ({persona.skills.length})</summary>
        <ul className="small">
          {persona.skills.map((s, i) => (
            <li key={i}>{s}</li>
          ))}
        </ul>
      </details>
      <p className="muted small">Generated {persona.generated_at} · {persona.memories_count} memories</p>
    </section>
  );
}
