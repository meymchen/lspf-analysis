use std::path::PathBuf;

use clap::Parser;

use enums::*;

#[derive(Parser, Debug)]
#[clap(
    name = "enums",
    version,
    author,
    about = "Generate Rust enums for the tree-sitter node kinds of every supported language."
)]
struct Opts {
    /// Output directory.
    #[clap(long, short, default_value = ".", value_parser)]
    output: PathBuf,
    /// File name template.
    #[clap(long, short, default_value = "language_$")]
    file_template: String,
}

fn main() {
    let opts = Opts::parse();

    if let Some(err) = generate_rust(&opts.output, &opts.file_template).err() {
        eprintln!("{:?}", err);
    }
}
