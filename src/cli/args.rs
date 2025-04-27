use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// YouTube URL to download
    #[arg(short, long)]
    pub url: String,

    /// Output format (mp3 or mp4)
    #[arg(short, long, default_value = "mp4")]
    pub format: String,

    /// Output directory
    #[arg(short, long, default_value = ".")]
    pub output_dir: PathBuf,

    /// Quality (best, worst, or specific resolution like 720)
    #[arg(short, long, default_value = "720")]
    pub quality: String,
}
