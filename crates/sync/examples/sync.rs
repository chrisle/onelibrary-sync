//! Syncs a stick from the command line: plans, then syncs what the plan saw.
//!
//! ```sh
//! cargo run -p onelibrary-sync --example sync -- /Volumes/DJ\ STICK
//! ```
#![allow(clippy::print_stdout, clippy::expect_used)]

fn main() {
    let mount = std::path::PathBuf::from(std::env::args().nth(1).expect("usage: sync <mount point>"));
    let planned = match onelibrary_sync::plan(&mount) {
        Ok(plan) => plan,
        Err(error) => return println!("cannot plan: {error}"),
    };
    println!("{:#?}", planned.changes);
    match onelibrary_sync::sync(&mount, &planned.fingerprint, &mut |step| println!("{}…", step.name())) {
        Ok(report) => println!("done in {} ms; wrote {:?}", report.elapsed_ms, report.changes.tables),
        Err(error) => println!("failed: {error}"),
    }
}
