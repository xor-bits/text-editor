use clap::Parser;

//

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// an initial file to be opened
    pub file: Option<String>,
}
