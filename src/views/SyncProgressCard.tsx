import type { SyncProgress } from "@/ipc/types";
import styles from "./SyncPane.module.css";

const DETAILS: Record<string, string> = {
  reading: "Reading the OneLibrary and the Device Library.",
  writing: "Writing the Device Library tables that changed.",
  verifying: "Checking the Device Library reads back as written.",
};

/** A sync in progress, drawn as RBXport Restore draws a restore in progress. */
export function SyncProgressCard({ progress, name }: { progress: SyncProgress; name: string }) {
  return <section className={`${styles.summary} ${styles.activeRestore}`} aria-label="Syncing">
    <div className={styles.restoreProgress}>
      <div className={styles.progressHeading} role="status" aria-live="polite">
        <strong>Syncing {name}</strong>
      </div>
      <progress className={styles.progressBar} aria-label="Sync progress" />
      <div className={styles.progressDetails}>{DETAILS[progress.phase] ?? DETAILS.reading}</div>
      <div className={styles.progressFooter}>
        <p>Keep the USB connected and rekordbox closed until this finishes.</p>
      </div>
    </div>
  </section>;
}
