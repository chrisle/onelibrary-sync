import { useEffect, useRef } from "react";
import { LoaderCircle } from "lucide-react";
import type { Device, Plan } from "@/ipc/types";
import type { PlanState } from "@/store/useDevices";
import { Button } from "@/components/controls";
import { formatBytes } from "@/lib/format";
import { anyChange, comparisonRows, describeFiles, headline, syncNotes, type ComparisonRow } from "@/lib/changes";
import styles from "./SyncPane.module.css";

function Comparison({ plan }: { plan: Plan }) {
  const rows = comparisonRows(plan);
  const counts = rows.filter(row => row.kind !== "setting");
  const settings = rows.filter(row => row.kind === "setting");
  const line = (row: ComparisonRow) => {
    const { difference } = row;
    return <tr key={row.label} data-changed={row.changed || undefined} className={row.kind === "own" ? styles.own : undefined}
      title={row.kind === "own" ? "Each library keeps the history its players record; it is not synced." : undefined}>
      <th scope="row">{row.label}</th>
      <td>{row.oneLibrary}</td>
      <td>
        {row.deviceLibrary}
        {row.change ? <small>{row.change}</small>
          : difference !== 0 ? <span className={styles.delta}>{difference > 0 ? "+" : "−"}{Math.abs(difference).toLocaleString()}</span> : null}
      </td>
    </tr>;
  };
  return <table className={styles.counts} aria-label="What each library holds">
    <thead><tr><th scope="col">Library</th><th scope="col">OneLibrary</th><th scope="col"><span aria-hidden="true">→ </span>Device Library</th></tr></thead>
    <tbody>{counts.map(line)}</tbody>
    {settings.length > 0 ? <tbody>
      <tr className={styles.group}><th scope="rowgroup" colSpan={3}>Settings that differ</th></tr>
      {settings.map(line)}
    </tbody> : null}
  </table>;
}

/**
 * A USB opened up: what its two libraries hold side by side, and what a
 * sync would change.
 */
export function DeviceDetails({ device, plan, blocked, focus, onSync, onRecover, onRecheck, onOpen }: {
  device: Device;
  plan: PlanState | undefined;
  /** Why a sync cannot start now, if it cannot. */
  blocked: string;
  /** Bring the sync into view, as the row's Sync… asks. */
  focus: boolean;
  onSync: () => void;
  onRecover: () => void;
  onRecheck: () => void;
  onOpen: () => void;
}) {
  const actions = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focus) actions.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [focus]);
  const facts = <p className={styles.facts}>
    <button type="button" role="link" className={styles.directoryLink} onClick={onOpen}>{device.path}</button>
    {[
      device.fileSystem,
      device.totalBytes > 0 ? `${formatBytes(device.freeBytes)} free of ${formatBytes(device.totalBytes)}` : "",
      plan?.state === "ready" && plan.plan.deviceName ? `Device name ${plan.plan.deviceName}` : "",
    ].filter(Boolean).map(fact => <span key={fact}>{fact}</span>)}
  </p>;

  if (!device.hasOneLibrary) {
    return <div className={styles.details}>
      <p className={styles.reading}>This volume has no OneLibrary. Export one to it from rekordbox 7, or another app that writes OneLibrary, then sync.</p>
      {facts}
    </div>;
  }
  if (device.interrupted) {
    return <div className={styles.details}>
      <p className={styles.notice}>
        An earlier write to this USB was interrupted before it finished. It has to be finished before the USB can be read or synced.
        <Button className={styles.backupButton} disabled={blocked !== ""} onClick={onRecover}>Finish now</Button>
      </p>
      {facts}
    </div>;
  }
  if (!plan || plan.state === "checking") {
    return <div className={styles.details}>
      <p className={styles.reading} role="status">
        <LoaderCircle size={16} className={styles.sizeSpinner} aria-hidden="true" />
        Reading this USB’s libraries.
      </p>
      {facts}
    </div>;
  }
  if (plan.state === "failed") {
    return <div className={styles.details}>
      <p className={styles.error} role="alert">{plan.message}</p>
      <div className={styles.restoreActions}><span /><Button className={styles.backupButton} onClick={onRecheck}>Check again</Button></div>
      {facts}
    </div>;
  }

  const { changes } = plan.plan;
  const changed = anyChange(changes);
  const notes = syncNotes(changes, plan.plan.deviceLibrary !== null, plan.plan.missingAudio, plan.plan.missingAnalysis);
  return <div className={styles.details} aria-label={`${device.name} libraries`}>
    <div className={styles.planBar} ref={actions}>
      <div>
        <p className={styles.headline} data-changed={changed || undefined}>{headline(plan.plan)}</p>
        {blocked ? <p className={styles.blockedReason} role="status">{blocked}</p> : null}
      </div>
      <div className={styles.actions}>
        <Button className={styles.backupButton} onClick={onRecheck}>Check again</Button>
        <Button className={styles.backupButton} disabled={!changed || blocked !== ""} onClick={onSync}>
          {changed ? "Sync now…" : "In sync"}
        </Button>
      </div>
    </div>
    <Comparison plan={plan.plan} />
    {notes.map(note => <p key={note} className={styles.partialNote}>{note}</p>)}
    {changed ? <p className={styles.files}>{describeFiles(changes)}</p> : null}
    {facts}
  </div>;
}
