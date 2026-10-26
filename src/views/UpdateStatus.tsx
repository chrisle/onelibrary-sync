import type { Updater } from "@/store/useUpdater";
import styles from "./SyncPane.module.css";

/** A download's progress as a whole percentage, or "" when its size is unknown. */
function percent(downloaded: number, total: number | null): string {
  return total ? ` ${Math.min(100, Math.floor((downloaded / total) * 100))}%` : "";
}

/**
 * Where updates stand, beside the version: a link to check, the download as
 * it goes, and Restart now once the new version is ready. Restarting waits
 * while a USB syncs, since it would cut the sync off.
 */
export function UpdateStatus({ updater, syncing }: { updater: Updater; syncing: boolean }) {
  const { state, automatic } = updater;
  const link = (text: string, onClick: () => void, disabled = false, title?: string) =>
    <button type="button" className={styles.directoryLink} disabled={disabled} title={title} onClick={onClick}>{text}</button>;
  switch (state.phase) {
    case "checking":
      return automatic ? link("Check for updates", updater.check, true) : <span>Checking for updates…</span>;
    case "upToDate":
      return automatic ? link("Check for updates", updater.check) : <span>Up to date.</span>;
    case "available":
    case "downloading":
      return <span>Downloading version {state.check.version}…{state.phase === "downloading" && state.progress
        ? percent(state.progress.downloaded, state.progress.total) : ""}</span>;
    case "installing":
      return <span>Installing version {state.check.version}…</span>;
    case "ready":
      return <span>
        Version {state.ready.version} is ready.{" "}
        {link("Restart now", updater.restart, syncing, syncing ? "Wait for the syncs to finish." : undefined)}
      </span>;
    case "failed":
      // A check the app ran on its own that failed is left for the next launch.
      return automatic && !state.check ? link("Check for updates", updater.check)
        : <span className={styles.updateFailed}>{state.message} {link("Try again", updater.retry)}</span>;
    default:
      return link("Check for updates", updater.check);
  }
}
