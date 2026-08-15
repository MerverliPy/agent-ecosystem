import { useState } from "react";
import type { ApprovalCard } from "../lib/types.ts";
import { decide, confidenceDelta, applyConfidence } from "../lib/approvals.ts";

export default function ApprovalCardView({ card }: { card: ApprovalCard }) {
  const [status, setStatus] = useState(card.status);

  if (status !== "pending") {
    return (
      <div className={`approval ${status}`}>
        <span className="muted small">{card.description}</span>
        <span className={`badge ${status}`}>{status}</span>
      </div>
    );
  }

  return (
    <div className="approval pending">
      <span className="small">{card.description}</span>
      <div className="approval-actions">
        <button
          className="approve"
          onClick={() => {
            setStatus("approved");
            // Learning signal recorded by the Rust core; demo applies the delta locally.
            if (card.event) applyConfidence(card.event, confidenceDelta("approved"));
          }}
        >
          Approve
        </button>
        <button
          className="reject"
          onClick={() => {
            setStatus("rejected");
            if (card.event) applyConfidence(card.event, confidenceDelta("rejected"));
          }}
        >
          Reject
        </button>
      </div>
    </div>
  );
}
