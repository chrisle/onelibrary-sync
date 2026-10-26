import { LoaderCircle } from "lucide-react";
import type { Device, Plan } from "@/ipc/types";
import type { PlanState } from "@/store/useDevices";
import { Button } from "@/components/controls";
import { formatBytes, formatFileSystem } from "@/lib/format";
import { anyChange, comparisonRows, syncNotes, type ComparisonRow } from "@/lib/changes";
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
export function DeviceDetails({ device, plan, blocked, onSync, onRecover, onRecheck, onOpen }: {
  device: Device;
  plan: PlanState | undefined;
  /** Why a sync cannot start now, if it cannot. */
  blocked: string;
  onSync: () => void;
  onRecover: () => void;
  onRecheck: () => void;
  onOpen: () => void;
}) {
  const used = device.totalBytes > 0 ? (device.totalBytes - device.freeBytes) / device.totalBytes : 0;
  const header = <div className={styles.detailsHeader}>
    <div className={styles.volume}>
      <strong>{device.name}</strong>
      {device.fileSystem ? <span className={styles.fileSystem}>{formatFileSystem(device.fileSystem)}</span> : null}
    </div>
    {device.totalBytes > 0 ? <div className={styles.space}>
      <div className={styles.capacityBar} role="meter" aria-label="Space used" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(used * 100)}>
        <span style={{ width: `${Math.round(used * 100)}%` }} />
      </div>
      <span>{formatBytes(device.freeBytes)} free of {formatBytes(device.totalBytes)}</span>
    </div> : <span />}
    {device.hasOneLibrary && !device.interrupted
      ? <Button className={styles.backupButton} disabled={plan?.state === "checking"} onClick={onRecheck}>Refresh</Button> : <span />}
  </div>;
  const stat = (value: string, label: string) => <span key={label}>{value} <span className={styles.statLabel}>{label}</span></span>;
  const volumeLink = <button type="button" role="link" className={styles.directoryLink} onClick={onOpen}>{device.path}</button>;

  if (!device.hasOneLibrary) {
    return <div className={styles.details}>
      {header}
      <p className={styles.reading}>This volume has no OneLibrary. Export one to it from rekordbox 7, or another app that writes OneLibrary, then sync.</p>
      <div className={styles.footer}><p className={styles.stats}>{volumeLink}</p></div>
    </div>;
  }
  if (device.interrupted) {
    return <div className={styles.details}>
      {header}
      <p className={styles.notice}>
        An earlier write to this USB was interrupted before it finished. It has to be finished before the USB can be read or synced.
        <Button className={styles.backupButton} disabled={blocked !== ""} onClick={onRecover}>Finish now</Button>
      </p>
      <div className={styles.footer}><p className={styles.stats}>{volumeLink}</p></div>
    </div>;
  }
  if (!plan || plan.state === "checking") {
    return <div className={styles.details}>
      {header}
      <p className={styles.reading} role="status">
        <LoaderCircle size={16} className={styles.sizeSpinner} aria-hidden="true" />
        Reading this USB’s libraries.
      </p>
      <div className={styles.footer}><p className={styles.stats}>{volumeLink}</p></div>
    </div>;
  }
  if (plan.state === "failed") {
    return <div className={styles.details}>
      {header}
      <p className={styles.error} role="alert">{plan.message}</p>
      <div className={styles.footer}><p className={styles.stats}>{volumeLink}</p></div>
    </div>;
  }

  const { changes } = plan.plan;
  const changed = anyChange(changes);
  const notes = syncNotes(changes, plan.plan.deviceLibrary !== null, plan.plan.missingAudio, plan.plan.missingAnalysis);
  const differences = comparisonRows(plan.plan).filter(row => row.changed).length;
  return <div className={styles.details} aria-label={`${device.name} libraries`}>
    {header}
    <Comparison plan={plan.plan} />
    {notes.map(note => <p key={note} className={styles.partialNote}>{note}</p>)}
    <div className={styles.footer}>
      <p className={styles.stats}>
        {changes.created ? stat("No", "Device Library yet") : stat(differences.toLocaleString(), differences === 1 ? "difference" : "differences")}
        {plan.plan.deviceName ? stat(plan.plan.deviceName, "device name") : null}
        {volumeLink}
      </p>
      <div className={styles.actions}>
        {blocked ? <span className={styles.blockedReason} role="status">{blocked}</span> : null}
        <Button className={styles.backupButton} disabled={!changed || blocked !== ""} onClick={onSync}>
          {changed ? "Sync now" : "In sync"}
        </Button>
      </div>
    </div>
  </div>;
}
