import { Fragment, useCallback, useRef, useState } from "react";
import { ChevronRight } from "lucide-react";
import { getBackend } from "@/ipc/client";
import type { Backend, Device, Status, SyncProgress } from "@/ipc/types";
import { Button, Section } from "@/components/controls";
import { errorMessage } from "@/lib/errorMessage";
import { formatBytes, formatCount, formatFileSystem } from "@/lib/format";
import { anyChange } from "@/lib/changes";
import { useDevices, type PlanState } from "@/store/useDevices";
import { useStatus } from "@/store/useStatus";
import { useSyncJob } from "@/store/useSyncJob";
import { DeviceDetails } from "./DeviceDetails";
import { SyncStep } from "./SyncStep";
import styles from "./SyncPane.module.css";

/** Why a sync cannot start now, or "". */
export function blockedReason(status: Status | null, running: boolean): string {
  if (running) return "A sync is running.";
  if (!status) return "Checking whether a sync can run…";
  if (status.rekordboxRunning) return "Quit rekordbox before syncing. It writes to the USBs it sees.";
  return "";
}

/** The row's status: what the list says about a USB at a glance. */
function rowState(device: Device, plan: PlanState | undefined): { text: string; state: string } {
  if (!device.hasOneLibrary) return { text: "No OneLibrary", state: "none" };
  if (device.interrupted) return { text: "Interrupted write", state: "failed" };
  if (!plan || plan.state === "checking") return { text: "Checking…", state: "checking" };
  if (plan.state === "failed") return { text: "Can’t read", state: "failed" };
  const { changes } = plan.plan;
  if (changes.created) return { text: "No Device Library yet", state: "changes" };
  return anyChange(changes) ? { text: "Needs sync", state: "changes" } : { text: "In sync", state: "synced" };
}

function nameIn(names: ReadonlyMap<string, string>, path: string | null): string {
  return (path && names.get(path)) ?? "The USB";
}

function finishedMessage(progress: SyncProgress, name: string): { text: string; failed: boolean } {
  switch (progress.phase) {
    case "complete": return progress.report && anyChange(progress.report.changes)
      ? { text: `${name} synced. Players that read the Device Library now see what its OneLibrary holds.`, failed: false }
      : { text: `${name} was already in sync. Nothing was written.`, failed: false };
    case "failed": return { text: progress.error ?? "The sync failed.", failed: true };
    default: return { text: "", failed: false };
  }
}

export function SyncPane() {
  const { status } = useStatus();
  const { devices, plans, loading, error: listError, reload, recheck } = useDevices();
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set());
  const running = useRef(false);
  // Kept after a USB goes, so a sync's end can still name it.
  const names = useRef(new Map<string, string>());
  for (const device of devices) names.current.set(device.path, device.name);

  const job = useSyncJob(useCallback((progress: SyncProgress) => {
    const done = finishedMessage(progress, nameIn(names.current, progress.path));
    if (done.failed) { setError(done.text); setMessage(""); } else { setMessage(done.text); setError(""); }
    // The Device Library has changed, or the USB did; read it again.
    if (progress.path) recheck(progress.path);
  }, [recheck]));

  const run = async (label: string, action: (backend: Backend) => Promise<string>) => {
    if (running.current) return;
    running.current = true;
    setBusy(label); setError(""); setMessage("");
    try { setMessage(await action(await getBackend())); }
    catch (e) { setError(errorMessage(e)); }
    finally { running.current = false; setBusy(""); }
  };
  const sync = (device: Device) => {
    const plan = plans.get(device.path);
    if (plan?.state !== "ready") return;
    void run("", async () => {
      await job.start(device.path, plan.plan.fingerprint);
      return "";
    });
  };
  const toggle = (path: string, open?: boolean) => setExpanded(current => {
    const next = new Set(current);
    if (open ?? !next.has(path)) next.add(path); else next.delete(path);
    return next;
  });

  const blocked = blockedReason(status, job.progress.running);
  const unavailable = busy !== "" || job.progress.running;
  return <Section title="OneLibrary 4 All" subtitle={`Version ${__APP_VERSION__}`}>
    {status?.rekordboxRunning ? <p className={styles.blockedNotice}>Quit rekordbox before syncing. It writes to the USBs it sees.</p> : null}
    {error || listError ? <p role="alert" className={styles.error}>{error || listError}</p> : null}

    <p className={styles.status} role="status" aria-live="polite" data-done={message ? true : undefined}>
      {job.progress.running ? `Keep ${nameIn(names.current, job.progress.path)} connected and rekordbox closed until the sync finishes.` : busy || message}
    </p>

    <div className={styles.listHeading}>
      <h4 className={styles.sectionHeading}>Connected USBs</h4>
      <Button className={styles.backupButton} disabled={loading} onClick={() => { setMessage(""); void reload(); }}>Refresh</Button>
    </div>
    {loading ? <p className={styles.status} role="status">Looking for USBs…</p> : null}
    {!loading && devices.length === 0 ? <div className={styles.empty}>
      <strong>No USBs connected.</strong>
      <p>Connect a USB that holds a OneLibrary. It is listed here as soon as it is mounted.</p>
    </div> : null}
    {devices.length > 0 ? <div className={styles.tableScroll}><table className={styles.table} aria-label="Connected USBs">
      <thead><tr><th><span hidden>Details</span></th><th>Name</th><th>Space</th><th>Tracks</th><th>Status</th><th>Actions</th></tr></thead>
      <tbody>{devices.map(device => {
        const open = expanded.has(device.path);
        const plan = plans.get(device.path);
        const state = rowState(device, plan);
        const ready = plan?.state === "ready" ? plan.plan : null;
        const used = device.totalBytes > 0 ? (device.totalBytes - device.freeBytes) / device.totalBytes : 0;
        const syncing = job.progress.running && job.progress.path === device.path;
        return <Fragment key={device.path}>
          <tr className={styles.row} data-expanded={open || undefined} onClick={() => toggle(device.path)}>
            <td>
              <button type="button" className={styles.expander} aria-expanded={open} aria-label={`${open ? "Hide" : "Show"} what ${device.name} holds`}
                onClick={event => { event.stopPropagation(); toggle(device.path); }}>
                <ChevronRight size={16} aria-hidden="true" />
              </button>
            </td>
            <td><div className={styles.deviceName}><span>{device.name}</span><small>{formatFileSystem(device.fileSystem)}</small></div></td>
            <td>{syncing ? <SyncStep phase={job.progress.phase} />
              : device.totalBytes > 0 ? <div className={styles.capacity} title={`${formatBytes(device.freeBytes)} free of ${formatBytes(device.totalBytes)}`}>
              <span>{formatBytes(device.freeBytes)} free</span>
              <div className={styles.capacityBar}><span style={{ width: `${Math.round(used * 100)}%` }} /></div>
            </div> : <span className={styles.dim}>—</span>}</td>
            <td className={styles.size}>{ready ? formatCount(ready.oneLibrary.tracks) : <span className={styles.dim}>—</span>}</td>
            <td className={styles.state} data-state={syncing ? "syncing" : state.state}>{syncing ? "Syncing…" : state.text}</td>
            <td onClick={event => event.stopPropagation()}><div className={styles.actions}>
              <Button className={styles.backupButton} disabled={unavailable || blocked !== "" || !ready || !anyChange(ready.changes)}
                onClick={() => sync(device)}>Sync</Button>
              <Button className={`${styles.backupButton} ${styles.deleteButton}`} disabled={unavailable} onClick={() => void run(`Ejecting ${device.name}…`, async b => {
                await b.ejectDevice(device.path); await reload(); return `${device.name} was ejected. It can be unplugged.`;
              })}>Eject</Button>
            </div></td>
          </tr>
          {open ? <tr className={styles.detailsRow}><td colSpan={6}>
            <DeviceDetails device={device} plan={plan} blocked={blocked}
              onSync={() => sync(device)}
              onRecheck={() => recheck(device.path)}
              onOpen={() => { setError(""); void getBackend().then(b => b.openDevice(device.path)).catch(e => setError(errorMessage(e))); }}
              onRecover={() => void run("Finishing the interrupted write…", async b => {
                await b.recoverDevice(device.path); await reload(); return `The interrupted write on ${device.name} was finished.`;
              })} />
          </td></tr> : null}
        </Fragment>;
      })}</tbody>
    </table></div> : null}
  </Section>;
}
