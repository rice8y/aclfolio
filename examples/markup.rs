//! Convert LaTeX markup without loading a metadata snapshot.
//!
//! Run with: cargo run --example markup

use aclfolio::MarkupText;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let text = MarkupText::from_latex(r"An \emph{example} of $x^2$", true)?;

    println!("Plain text: {}", text.as_text()?);
    println!("HTML: {}", text.as_html(true)?);
    println!("XML: {}", text.as_xml());

    Ok(())
}
