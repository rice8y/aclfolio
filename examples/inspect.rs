//! Summarize a local ACL Anthology metadata directory.
//!
//! Run with: cargo run --example inspect -- DATA

use aclfolio::Anthology;
use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let data_directory = env::args_os()
        .nth(1)
        .ok_or("usage: cargo run --example inspect -- DATA")?;
    let anthology = Anthology::open(data_directory)?;

    println!("Collections: {}", anthology.collections().len());
    println!("Volumes: {}", anthology.volumes().len());
    println!("Papers: {}", anthology.papers().len());
    println!("Events: {}", anthology.events().len());
    println!("People: {}", anthology.people().len());
    println!("Diagnostics: {}", anthology.diagnostics().len());

    Ok(())
}
