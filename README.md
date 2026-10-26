<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="OneLibrary Sync logo" width="128" height="128">
</p>

# OneLibrary Sync

OneLibrary Sync copies the OneLibrary on a USB stick to the Device Library on
the same stick, so the stick can be used with older Pioneer DJ equipment that
reads only the Device Library, such as the CDJ-2000NXS2 and XDJ-1000MK2.

It is built with Tauri 2, Rust and React for macOS, Windows and Linux, and is
licensed GPL-2.0-or-later.

## What it does

- Lists every connected volume and notices a USB being plugged in or pulled
  out within two seconds. Each one with a OneLibrary is read straight away and
  shows whether it is in sync.
- Opening a USB (the arrow) shows what its two libraries hold side by side:
  tracks, playlists, folders, artists, albums, genres, labels, keys, artwork
  and My Tags, with differences highlighted, and what a sync would change.
- Tick the box beside each USB to sync and press **Sync** under the list. The
  ticked USBs sync at the same time, one way, from the OneLibrary to the Device
  Library, without asking again. The OneLibrary is only read.
- With **Eject when done** ticked, each USB that synced is ejected as soon as
  it finishes. One whose sync failed stays connected.
- The eject icon in a USB's row hands it back to the system so it can be
  unplugged.
- Updates install themselves, as RBXport's do. A few seconds after launch the
  app checks for a newer version and downloads it. On macOS and with the
  AppImage it is put in place at once and the next launch runs it; on Windows
  it installs when the app quits. **Restart now**, beside the version, runs it
  straight away once no USB is syncing. **Check for updates** checks by hand.

Only the USB is read and written. The rekordbox library on the computer is
never opened.

## What a sync writes

The audio, analysis and artwork files are already on the USB, and both
libraries name the same files, so a sync writes only the Device Library's
databases, and of those only what differs:

- Each of the fourteen tables of `export.pdb` that a OneLibrary fills is
  compared with what the USB holds, decoded the way a player reads it. A
  table that reads the same is left byte for byte as it is; one that differs
  is rewritten in place, leaving every other table's pages untouched.
- `exportExt.pdb`, the My Tags, is written only when the tags or the tracks
  carrying them differ.
- A USB that is already in sync is not written at all.

A track the Device Library already holds keeps the fields rekordbox writes
differently into its two libraries (the master library's ids, the file size
and the file's creation date), so a USB rekordbox exported with both
libraries reads as in sync. The play history players record in the Device
Library is the USB's own and is kept.

Playlists, folders and tracks that only the Device Library has, such as a
playlist saved on a player, are removed. The opened USB says so when that
happens.

## Safety

- Quit rekordbox before syncing; it writes to the USBs it sees.
- The sync reads the USB again and compares it with what the window
  showed. If the USB changed in between, or changes while the new files are
  being prepared, nothing is written.
- Files are written through RBXport's publication journal and write lock on
  the USB (`.rbxport-publication`, `.rbxport-write.lock`). This app and RBXport
  never write a USB at the same time, and an interrupted write is finished
  before the USB is read again, by either app.
- The new `export.pdb` is decoded and checked against the OneLibrary before
  it is published, and read back after.

## Development

The database formats, the durable publication and the volume handling come
from RBXport's library crates. Pinned snapshots of those crates live under
`crates/` (`rbl-core`, `rbl-anlz`, `rbl-pdb` and `rbl-onelibrary` from rbxport
`c79284c1`; `rbl-devices` trimmed to listing, watching and ejecting). The sync
itself is `crates/sync`.

```sh
pnpm install
pnpm dev          # the app
pnpm dev:web      # the window in a browser, with a stand-in backend
```

```sh
pnpm test && pnpm typecheck && pnpm lint
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

In the browser, `?rekordbox=1` has rekordbox running, `?interrupted=1` leaves
an interrupted write on the first USB, `?fail=1` fails syncs, `?empty=1`
connects nothing and `?late=1` connects another USB after a few seconds.

Tests on a real USB stick run by hand, never in CI. They rewrite the stick's
libraries and put the files back after each test, pass or fail. Use a stick
whose libraries may be rewritten, and quit rekordbox first:

```sh
ONELIBRARY_TEST_USB="/Volumes/USB B" cargo test -p onelibrary-sync --test usb -- --ignored
```

To try the real app on a copy of a USB, set `RB_LITE_FAKE_VOLUMES` to a
`:`-separated list of folders that stand in for volumes. Two command-line
examples work on any mounted USB or folder:

```sh
cargo run -p onelibrary-sync --example plan -- /Volumes/DJ\ STICK --diff
cargo run -p onelibrary-sync --example sync -- /Volumes/DJ\ STICK
```

## Releases

Pushing a version tag such as `v0.1.0` runs the release workflow on the arc
self-hosted fleet. It builds signed and notarized macOS DMGs for Apple Silicon
and Intel, a signed Windows installer, and Linux AppImage and Debian packages.

Release files share RBXport's R2 bucket under the isolated
`onelibrary-sync-<version>-<platform>` namespace. The current download manifest
is `onelibrary-sync-latest.json`; RBXport's own `latest.json` is never changed.
A manual workflow run without a tag builds preview artifacts without publishing.

The manifest is also the update feed installed copies read. Each platform's
update bundle is signed with the OneLibrary Sync updater key (1Password, Code
vault), whose public half is `plugins.updater.pubkey` in
`src-tauri/tauri.conf.json`; the workflow needs its private key and password as
the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
secrets. The Windows installer installs for the current user, so an update
installs without an administrator prompt.

## License

GPL-2.0-or-later. See [LICENSE](LICENSE) and [COPYING](COPYING).
