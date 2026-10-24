//! Compare paper records in two local metadata snapshots.
//!
//! Run with: cargo run --example changes -- OLD_DATA NEW_DATA

use aclfolio::{Anthology, LoadOptions};
use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let old_directory = arguments
        .next()
        .ok_or("usage: cargo run --example changes -- OLD_DATA NEW_DATA")?;
    let new_directory = arguments
        .next()
        .ok_or("usage: cargo run --example changes -- OLD_DATA NEW_DATA")?;

    let original = Anthology::open(old_directory)?;
    let updated = original.reload(new_directory, &LoadOptions::default())?;
    let changes = original.diff(&updated).papers;

    println!(
        "Papers: {} added, {} modified, {} removed",
        changes.added.len(),
        changes.modified.len(),
        changes.removed.len()
    );
    for (label, paper_ids) in [
        ("Added", &changes.added),
        ("Modified", &changes.modified),
        ("Removed", &changes.removed),
    ] {
        for paper_id in paper_ids {
            println!("{label}: {paper_id}");
        }
    }

    Ok(())
}
