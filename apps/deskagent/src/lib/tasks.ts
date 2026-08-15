// Scheduled tasks placeholder (Phase 6 Task 6): pure logic for a minimal task list.
import { newId, nowIso } from "./sessions.ts";

export type TaskStatus = "scheduled" | "due" | "done" | "cancelled";

export interface ScheduledTask {
  id: string;
  name: string;
  cadence: "once" | "daily" | "weekly";
  next_run: string; // ISO date-time
  status: TaskStatus;
}

export function createTask(name: string, cadence: ScheduledTask["cadence"], nextRunIso?: string): ScheduledTask {
  return {
    id: newId("task"),
    name,
    cadence,
    next_run: nextRunIso ?? nowIso(),
    status: "scheduled",
  };
}

export function isDue(task: ScheduledTask, nowIsoStr: string): boolean {
  if (task.status !== "scheduled") return false;
  return task.next_run <= nowIsoStr;
}

export function markDone(task: ScheduledTask): ScheduledTask {
  return { ...task, status: "done" };
}

/** Roll a due task to its next occurrence. */
export function roll(task: ScheduledTask, nowIsoStr: string): ScheduledTask {
  const next = new Date(nowIsoStr);
  if (task.cadence === "daily") next.setDate(next.getDate() + 1);
  else if (task.cadence === "weekly") next.setDate(next.getDate() + 7);
  return { ...task, next_run: next.toISOString(), status: "scheduled" };
}
