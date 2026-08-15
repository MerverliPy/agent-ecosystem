import { useState } from "react";
import type { ApprovalCard } from "../lib/types.ts";

export default function ApprovalCardView({
  card,
  onDecide,
}: {
  card: ApprovalCard;
  onDecide?: (cardId: string, approved: boolean) => Promise<void> | void;
}) {
  const [status, setStatus] = useState(card.status);
  const [busy, setBusy] = useState(false);

  if (status !== "pending") {
    return (
      <div className={`approval ${status}`}>
        <span className="muted small">{card.description}</span>
        <span className={`badge ${status}`}>{status}</span>
      </div>
    );
  }

  const decide = async (approved: boolean) => {
    if (busy) return;
    setBusy(true);
    try {
      // persist the decision to the core (records the +/- learning signal) when wired;
      // otherwise fall back to the local-only demo behavior.
      if (onDecide) await onDecide(card.id, approved);
      setStatus(approved ? "approved" : "rejected");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="approval pending">
      <span className="small">{card.description}</span>
      <div className="approval-actions">
        <button className="approve" disabled={busy} onClick={() => decide(true)}>
          Approve
        </button>
        <button className="reject" disabled={busy} onClick={() => decide(false)}>
          Reject
        </button>
      </div>
    </div>
  );
}
