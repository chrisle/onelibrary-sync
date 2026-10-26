import { Fragment, useEffect, useRef } from "react";
import { LoaderCircle } from "lucide-react";
import type { Counts, Device } from "@/ipc/types";
import type { PlanState } from "@/store/useDevices";
import { Button } from "@/components/controls";
import { formatBytes, formatCount } from "@/lib/format";
import { COUNT_ROWS, anyChange, describeCounts, describeTables, syncNotes } from "@/lib/changes";
import styles from "./SyncPane.module.css";

function Delta({ want, now }: { want: number; now: number | undefined }) {
  if (now === undefined) return <td className={styles.dim}>—</td>;
  const difference = now - want;
  return <td className={difference === 0 ? undefined : styles.changed}>
    {formatCount(now)}
    {difference === 0 ? null : <span className={styles.delta}>{difference > 0 ? "+" : "−"}{formatCount(Math.abs(difference))}</span>}
  </td>;
}

function Comparison({ one, legacy }: { one: Counts; legacy: Counts | null }) {
  return <table className={styles.counts} aria-label="What each library holds">
    <thead><tr><th scope="col">Library</th><th scope="col">OneLibrary</th><th scope="col">Device Library</th></tr></thead>
    <tbody>
      {COUNT_ROWS.map(row => <tr key={row.key}>
        <th scope="row">{row.label}</th>
        <td>{formatCount(one[row.key])}</td>
        <Delta want={one[row.key]} now={legacy?.[row.key]} />
      </tr>)}
      <tr className={styles.own} title="Each library keeps the history its players record; it is not synced.">
        <th scope="row">History playlists</th>
        <td>{formatCount(one.histories)}</td>
        <td>{legacy ? formatCount(legacy.histories) : "—"}</td>
      </tr>
    </tbody>
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
  const facts = <dl className={styles.facts}>
    <dt>Volume</dt>
    <dd><button type="button" role="link" className={styles.directoryLink} onClick={onOpen}>{device.path}</button></dd>
    <dt>Format</dt><dd>{device.fileSystem || "—"}</dd>
    {device.totalBytes > 0 ? <><dt>Space</dt><dd>{formatBytes(device.freeBytes)} free of {formatBytes(device.totalBytes)}</dd></> : null}
    {plan?.state === "ready" && plan.plan.deviceName ? <><dt>Device name</dt><dd>{plan.plan.deviceName}</dd></> : null}
  </dl>;

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
    <div className={styles.detailGrid}>
      <div>
        <Comparison one={plan.plan.oneLibrary} legacy={plan.plan.deviceLibrary} />
        {facts}
      </div>
      <div>
        <h5 className={styles.sectionHeading}>What a sync changes</h5>
        <dl className={styles.changeList}>
          {[
            { label: "Tracks", changed: changes.tracksAdded + changes.tracksUpdated + changes.tracksRemoved > 0,
              text: describeCounts(changes.tracksAdded, changes.tracksUpdated, changes.tracksRemoved) },
            { label: "Playlists and folders", changed: changes.playlistsAdded + changes.playlistsUpdated + changes.playlistsRemoved > 0,
              text: describeCounts(changes.playlistsAdded, changes.playlistsUpdated, changes.playlistsRemoved) },
            { label: "export.pdb", changed: changes.tables.length > 0, text: describeTables(changes) },
            { label: "My Tags", changed: changes.myTags, text: changes.myTags ? "exportExt.pdb rewritten" : "Left as it is" },
            { label: "OneLibrary", changed: false, text: "Read only, never changed" },
          ].map(line => <Fragment key={line.label}>
            <dt data-changed={line.changed || undefined}>{line.label}</dt>
            <dd data-changed={line.changed || undefined}>{line.text}</dd>
          </Fragment>)}
        </dl>
        {notes.map(note => <p key={note} className={styles.partialNote}>{note}</p>)}
        <div className={styles.restoreActions} ref={actions}>
          <p role="status">{blocked || (changed ? "" : "The Device Library already holds what the OneLibrary holds.")}</p>
          <div className={styles.actions}>
            <Button className={styles.backupButton} onClick={onRecheck}>Check again</Button>
            <Button className={styles.backupButton} disabled={!changed || blocked !== ""} onClick={onSync}>
              {changed ? "Sync now…" : "In sync"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  </div>;
}
