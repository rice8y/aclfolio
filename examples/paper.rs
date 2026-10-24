//! Look up a paper, resolve its authors and format citations.
//!
//! Run with: cargo run --example paper -- DATA PAPER_ID

use aclfolio::Anthology;
use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let data_directory = arguments
        .next()
        .ok_or("usage: cargo run --example paper -- DATA PAPER_ID")?;
    let paper_id = arguments
        .next()
        .ok_or("usage: cargo run --example paper -- DATA PAPER_ID")?;

    let anthology = Anthology::open(data_directory)?;
    let paper = anthology
        .get_paper(&paper_id)?
        .ok_or_else(|| format!("paper not found: {paper_id}"))?;

    println!("Title: {}", paper.title().as_text()?);
    println!("Year: {}", paper.year());
    println!("URL: {}", paper.canonical_url());
    if let Some(pdf) = paper.pdf() {
        println!("PDF: {}", pdf.url(anthology.config()));
    }

    println!("\nAuthors:");
    for credit in paper.author_credits() {
        let person = credit.resolve()?;
        let name = person.canonical_name()?.as_full();
        println!("  {name} ({})", person.id);
    }

    println!("\nBibTeX:\n{}", paper.to_bibtex(true)?);
    println!("\nMarkdown citation:\n{}", paper.to_markdown_citation()?);

    Ok(())
}
