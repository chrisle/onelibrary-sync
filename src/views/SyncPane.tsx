import { Fragment, useCallback, useRef, useState } from "react";
import { ChevronRight } from "lucide-react";
import { getBackend } from "@/ipc/client";
import type { Backend, Device, Status, SyncProgress } from "@/ipc/types";
import { Button, Checkbox, Section } from "@/components/controls";
import { errorMessage } from "@/lib/errorMessage";
import { formatBytes, formatCount, formatFileSystem } from "@/lib/format";
import { anyChange } from "@/lib/changes";
import { useDevices, type PlanState } from "@/store/useDevices";
import { useStatus } from "@/store/useStatus";
import { useSyncJobs } from "@/store/useSyncJobs";
import { DeviceDetails } from "./DeviceDetails";
import { SyncStep } from "./SyncStep";
import styles from "./SyncPane.module.css";

/** Why a USB's sync cannot start now, or "". `syncing` is whether that USB is syncing already. */
export function blockedReason(status: Status | null, syncing: boolean): string {
  if (syncing) return "This USB is being synced.";
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
    case "failed": return { text: `${name} was not synced. ${progress.error ?? "The sync failed."}`, failed: true };
    default: return { text: "", failed: false };
  }
}

/** "A", "A and B", "A, B and C". */
function listNames(names: readonly string[]): string {
  return names.length < 2 ? names.join("") : `${names.slice(0, -1).join(", ")} and ${names.at(-1) ?? ""}`;
}

/** Adds a sentence to a line that may already hold some. */
function append(line: string, text: string): string {
  return line ? `${line} ${text}` : text;
}

export function SyncPane() {
  const { status } = useStatus();
  const { devices, plans, loading, error: listError, reload, recheck } = useDevices();
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set());
  const [chosen, setChosen] = useState<ReadonlySet<string>>(() => new Set());
  const running = useRef(false);
  // Kept after a USB goes, so a sync's end can still name it.
  const names = useRef(new Map<string, string>());
  for (const device of devices) names.current.set(device.path, device.name);

  // USBs syncing side by side can finish together, so each one's outcome is
  // added to the line rather than replacing another's.
  const job = useSyncJobs(useCallback((progress: SyncProgress) => {
    const done = finishedMessage(progress, nameIn(names.current, progress.path));
    if (done.failed) setError(line => append(line, done.text)); else setMessage(line => append(line, done.text));
    // The Device Library has changed, or the USB did; read it again.
    if (progress.path) recheck(progress.path);
  }, [recheck]));
  const syncing = (path: string) => job.jobs.get(path)?.running ?? false;
  const syncingNames = [...job.jobs.values()].filter(progress => progress.running).map(progress => nameIn(names.current, progress.path));

  const run = async (label: string, action: (backend: Backend) => Promise<string>) => {
    if (running.current) return;
    running.current = true;
    setBusy(label); setError(""); setMessage("");
    try { setMessage(await action(await getBackend())); }
    catch (e) { setError(errorMessage(e)); }
    finally { running.current = false; setBusy(""); }
  };
  /** The plan a sync of `device` would carry out, if it can start. One already in sync writes nothing. */
  const syncable = (device: Device) => {
    const plan = plans.get(device.path);
    return plan?.state === "ready" && !syncing(device.path) ? plan.plan : null;
  };
  /** Starts each of `picked` side by side; one that cannot start does not hold up the rest. */
  const syncEach = (picked: readonly Device[]) => {
    const starting = picked.flatMap(device => {
      const plan = syncable(device);
      return plan ? [{ device, plan }] : [];
    });
    if (starting.length === 0) return;
    setChosen(current => {
      const next = new Set(current);
      for (const { device } of starting) next.delete(device.path);
      return next;
    });
    void run("", async () => {
      const results = await Promise.allSettled(starting.map(({ device, plan }) => job.start(device.path, plan.fingerprint)));
      const refused = results.flatMap((result, i) => result.status === "rejected"
        ? [starting.length === 1 ? errorMessage(result.reason) : `${starting[i]?.device.name}: ${errorMessage(result.reason)}`] : []);
      if (refused.length > 0) throw new Error(refused.join(" "));
      return "";
    });
  };
  const sync = (device: Device) => syncEach([device]);
  const choose = (path: string, on: boolean) => setChosen(current => {
    const next = new Set(current);
    if (on) next.add(path); else next.delete(path);
    return next;
  });
  const toggle = (path: string, open?: boolean) => setExpanded(current => {
    const next = new Set(current);
    if (open ?? !next.has(path)) next.add(path); else next.delete(path);
    return next;
  });

  const blocked = blockedReason(status, false);
  const unavailable = busy !== "";
  const toSync = devices.filter(device => chosen.has(device.path) && syncable(device));
  return <Section title="OneLibrary 4 All" subtitle={`Version ${__APP_VERSION__}`}>
    {status?.rekordboxRunning ? <p className={styles.blockedNotice}>Quit rekordbox before syncing. It writes to the USBs it sees.</p> : null}
    {error || listError ? <p role="alert" className={styles.error}>{error || listError}</p> : null}

    <p className={styles.status} role="status" aria-live="polite" data-done={message ? true : undefined}>
      {syncingNames.length > 0
        ? `Keep ${listNames(syncingNames)} connected and rekordbox closed until ${syncingNames.length === 1 ? "the sync finishes" : "the syncs finish"}.`
        : busy || message}
    </p>

    <div className={styles.listHeading}>
      <h4 className={styles.sectionHeading}>Connected USBs</h4>
      <div className={styles.actions}>
        {toSync.length > 0 ? <Button className={styles.backupButton} disabled={unavailable || blocked !== ""}
          label={`Sync all ${toSync.length} chosen USBs`} onClick={() => syncEach(toSync)}>Sync all</Button> : null}
        <Button className={styles.backupButton} disabled={loading} onClick={() => { setMessage(""); void reload(); }}>Refresh</Button>
      </div>
    </div>
    {loading ? <p className={styles.status} role="status">Looking for USBs…</p> : null}
    {!loading && devices.length === 0 ? <div className={styles.empty}>
      <strong>No USBs connected.</strong>
      <p>Connect a USB that holds a OneLibrary. It is listed here as soon as it is mounted.</p>
    </div> : null}
    {devices.length > 0 ? <div className={styles.tableScroll}><table className={styles.table} aria-label="Connected USBs">
      <thead><tr><th><span hidden>Details</span></th><th><span hidden>Choose</span></th><th>Name</th><th>Space</th><th>Tracks</th><th>Status</th><th>Actions</th></tr></thead>
      <tbody>{devices.map(device => {
        const open = expanded.has(device.path);
        const plan = plans.get(device.path);
        const state = rowState(device, plan);
        const ready = plan?.state === "ready" ? plan.plan : null;
        const used = device.totalBytes > 0 ? (device.totalBytes - device.freeBytes) / device.totalBytes : 0;
        const running = syncing(device.path);
        const canSync = syncable(device) !== null;
        const needsSync = canSync && ready !== null && anyChange(ready.changes);
        return <Fragment key={device.path}>
          <tr className={styles.row} data-expanded={open || undefined} onClick={() => toggle(device.path)}>
            <td>
              <button type="button" className={styles.expander} aria-expanded={open} aria-label={`${open ? "Hide" : "Show"} what ${device.name} holds`}
                onClick={event => { event.stopPropagation(); toggle(device.path); }}>
                <ChevronRight size={16} aria-hidden="true" />
              </button>
            </td>
            <td onClick={event => event.stopPropagation()}>
              <Checkbox checked={canSync && chosen.has(device.path)} disabled={!canSync} label={`Choose ${device.name} to sync`}
                onChange={on => choose(device.path, on)} />
            </td>
            <td><div className={styles.deviceName}><span>{device.name}</span><small>{formatFileSystem(device.fileSystem)}</small></div></td>
            <td>{running ? <SyncStep phase={job.jobs.get(device.path)?.phase ?? "reading"} />
              : device.totalBytes > 0 ? <div className={styles.capacity} title={`${formatBytes(device.freeBytes)} free of ${formatBytes(device.totalBytes)}`}>
              <span>{formatBytes(device.freeBytes)} free</span>
              <div className={styles.capacityBar}><span style={{ width: `${Math.round(used * 100)}%` }} /></div>
            </div> : <span className={styles.dim}>—</span>}</td>
            <td className={styles.size}>{ready ? formatCount(ready.oneLibrary.tracks) : <span className={styles.dim}>—</span>}</td>
            <td className={styles.state} data-state={running ? "syncing" : state.state}>{running ? "Syncing…" : state.text}</td>
            <td onClick={event => event.stopPropagation()}><div className={styles.actions}>
              <Button className={styles.backupButton} disabled={unavailable || blocked !== "" || !needsSync}
                onClick={() => sync(device)}>Sync</Button>
              <Button className={`${styles.backupButton} ${styles.deleteButton}`} disabled={unavailable || running} onClick={() => void run(`Ejecting ${device.name}…`, async b => {
                await b.ejectDevice(device.path); await reload(); return `${device.name} was ejected. It can be unplugged.`;
              })}>Eject</Button>
            </div></td>
          </tr>
          {open ? <tr className={styles.detailsRow}><td colSpan={7}>
            <DeviceDetails device={device} plan={plan} blocked={blockedReason(status, running)}
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
