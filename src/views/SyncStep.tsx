import type { SyncPhase } from "@/ipc/types";
import styles from "./SyncPane.module.css";

/** The steps of a sync, in order, as the row names them. */
const STEPS: Partial<Record<SyncPhase, string>> = {
  reading: "Reading libraries",
  writing: "Writing Device Library",
  verifying: "Checking what was written",
};
const ORDER = Object.keys(STEPS);

/** A sync in progress, in place of the row's space bar: the step it is on, and how many of the three are done. */
export function SyncStep({ phase }: { phase: SyncPhase }) {
  const at = Math.max(0, ORDER.indexOf(phase));
  return <div className={styles.capacity} title="Keep the USB connected and rekordbox closed until this finishes.">
    <span>{STEPS[phase] ?? STEPS.reading}… <span className={styles.dim}>{at + 1} of {ORDER.length}</span></span>
    <progress className={styles.syncBar} value={at + 1} max={ORDER.length} aria-label="Sync progress" />
  </div>;
}
