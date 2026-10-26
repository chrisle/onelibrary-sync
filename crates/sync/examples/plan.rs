//! Prints what syncing a stick would change, without writing anything, and
//! with `--diff` the first few differences in each table that changes.
//!
//! ```sh
//! cargo run -p onelibrary-sync --example plan -- /Volumes/DJ\ STICK --diff
//! ```
#![allow(clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use onelibrary_sync::{device, layout, plan, read_one_library};

fn main() {
    let mut args = std::env::args().skip(1);
    let mount = PathBuf::from(args.next().expect("usage: plan <mount point> [--diff]"));
    let diff = args.next().as_deref() == Some("--diff");
    match plan(&mount) {
        Ok(plan) => println!("{plan:#?}"),
        Err(error) => println!("cannot plan: {error}"),
    }
    if !diff {
        return;
    }
    let layout = layout(&mount).expect("no OneLibrary");
    let one = read_one_library(&layout).expect("OneLibrary");
    let existing = std::fs::read(layout.device_library()).ok();
    let Some(update) = device::update(&one, existing.as_deref(), "2026-01-01").expect("update") else {
        println!("in step");
        return;
    };
    let Some(before) = update.before else { return };
    let after = update.after;
    for (id, track) in &after.tracks {
        match before.tracks.get(id) {
            Some(was) if was != track => println!("track {id}:\n  stick {was:?}\n  wants {track:?}"),
            None => println!("track {id} added: {}", track.title),
            _ => {}
        }
    }
    for id in before.tracks.keys().filter(|id| !after.tracks.contains_key(id)) {
        println!("track {id} removed");
    }
    for (label, was, now) in [
        ("genres", format!("{:?}", before.genres), format!("{:?}", after.genres)),
        ("artists", format!("{:?}", before.artists), format!("{:?}", after.artists)),
        ("labels", format!("{:?}", before.labels), format!("{:?}", after.labels)),
        ("keys", format!("{:?}", before.keys), format!("{:?}", after.keys)),
        ("colors", format!("{:?}", before.colors), format!("{:?}", after.colors)),
        ("artwork", format!("{:?}", before.artwork), format!("{:?}", after.artwork)),
        ("albums", format!("{:?}", before.albums), format!("{:?}", after.albums)),
        ("tree", format!("{:?}", before.tree), format!("{:?}", after.tree)),
        ("entries", format!("{:?}", before.entries), format!("{:?}", after.entries)),
        ("columns", format!("{:?}", before.columns), format!("{:?}", after.columns)),
        ("categories", format!("{:?}", before.categories), format!("{:?}", after.categories)),
        ("sorts", format!("{:?}", before.sorts), format!("{:?}", after.sorts)),
        ("property", format!("{:?}", before.property), format!("{:?}", after.property))] {
        if was != now {
            println!("{label}:\n  stick {was}\n  wants {now}");
        }
    }
}
