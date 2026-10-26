/** `1.4 GB`, `312.0 MB`: sizes as RBXport's Backups pane shows them. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "";
  const gb = bytes / 1024 ** 3;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
}

/** As {@link formatBytes}, with bytes and kilobytes for the small parts of a chart. */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  return formatBytes(bytes);
}

export function formatCount(count: number): string {
  return count.toLocaleString();
}
