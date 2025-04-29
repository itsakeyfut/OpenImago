use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::time::Duration;
use yt_dlp::Youtube;
use yt_dlp::fetcher::deps::Libraries;

use yt_downloader::cli::args::Args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let libs_dir = PathBuf::from("libs");
    let output_dir = args.output_dir.clone();

    std::fs::create_dir_all(&libs_dir).context("Failed to create libraries directory")?;
    std::fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

    println!("Initializing downloader...");

    // Binary path for Windows
    let yt_path = libs_dir.join("yt-dlp.exe");
    let ffmpeg_path = libs_dir.join("ffmpeg.exe");
    // Not to move after procedure
    let yt_path_clone = yt_path.clone();

    // .zip file exitsts, but .exe not
    let ffmpeg_zip_path = libs_dir.join("ffmpeg-release.zip");

    // check if necessary binaries exist
    let mut yt_dlp_exists = yt_path_clone.exists();
    let mut ffmpeg_exists = ffmpeg_path.exists();

    // loop til both binaries exist
    while !yt_dlp_exists || !ffmpeg_exists {
        // yt-dlp.exe
        if !yt_dlp_exists {
            println!("yt-dlp.exe is missing. Trying to download...");

            let pb = create_spinner_progress_bar("Downloading yt-dlp.exe");

            let temp_output_dir = output_dir.clone();
            match Youtube::with_new_binaries(libs_dir.clone(), temp_output_dir).await {
                Ok(_) => {
                    pb.finish_with_message("yt-dlp.exe downloaded successfully");
                    if yt_path_clone.exists() {
                        yt_dlp_exists = true;
                    } else {
                        pb.finish_with_message("Could not find yt-dlp.exe after download");
                        return Err(anyhow!("Could not download yt-dlp.exe"));
                    }
                }
                Err(e) => {
                    pb.finish_with_message("Failed to download yt-dlp.exe");
                    println!("Error: {}", e);
                    println!(
                        "Please download yt-dlp.exe manually and place it in the libs directory"
                    );
                    return Err(anyhow!("Failed to download yt-dlp.exe"));
                }
            }
        }

        // ffmpeg.exe
        if !ffmpeg_exists {
            if ffmpeg_zip_path.exists() {
                println!("Found ffmpeg zip file. Extracting...");
                extract_ffmpeg_zip(&ffmpeg_zip_path, &libs_dir)?;

                println!("Searching for ffmpeg.exe in subdirectories...");
                match find_ffmpeg_in_dir(&libs_dir) {
                    Ok(ffmpeg_path_found) => {
                        println!("Found ffmpeg.exe at: {}", ffmpeg_path_found.display());
                        std::fs::copy(&ffmpeg_path_found, &ffmpeg_path)
                            .context("Failed to copy ffmpeg.exe to libs directory")?;
                        println!("Copied ffmpeg.exe to: {}", ffmpeg_path.display());

                        cleanup_ffmpeg_files(&libs_dir, &ffmpeg_zip_path)?;

                        ffmpeg_exists = true;
                    }
                    Err(e) => {
                        eprintln!("Error finding ffmpeg.exe: {}", e);
                        eprintln!(
                            "Please download ffmpeg.exe manually and place it in the libs directory"
                        );
                        return Err(anyhow!("Could not find ffmpeg.exe after extraction"));
                    }
                }
            } else {
                eprintln!("ffmpeg.exe and ffmpeg-release.zip are missing");
                eprintln!(
                    "Please download ffmpeg.exe or ffmpeg-release.zip and place it in the libs directory"
                );
                return Err(anyhow!("ffmpeg.exe is missing"));
            }
        }
    }

    println!("All required binaries are ready.");

    // Initialize Fetcher here because fail if binaries do not exist
    let libraries = Libraries::new(yt_path_clone, ffmpeg_path);
    Youtube::new(libraries, output_dir.clone()).context("Failed to initialize Youtube fetcher")?;

    println!("Downloading from URL: {}", args.url);

    // TODO: find video title from URL and name it
    let output_filename = match args.format.as_str() {
        "mp3" => format!("audio_{}.mp3", get_safe_timestamp()),
        "mp4" => {
            let quality_str = match args.quality.as_str() {
                "best" => "best",
                "worst" => "worst",
                res => res,
            };
            format!("video_{}_{}.mp4", quality_str, get_safe_timestamp())
        }
        other => return Err(anyhow!("Unsupported format: {}", other)),
    };

    let pb = create_download_progress_bar(&format!("Downloading {} file", args.format));

    // Update progress bar periodically
    let pb_clone = pb.clone();
    let progress_task = tokio::spawn(async move {
        for i in 1..=95 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            pb_clone.set_position(i);
        }
    });

    match cleanup_cache_files() {
        Ok(_) => {
            println!("Successfully removed cache directory");
        }
        Err(_) => {
            eprintln!("Err");
            return Err(anyhow!("Failed to remove cache directory"));
        }
    };

    // Start downloading
    let result = match args.format.as_str() {
        "mp3" => {
            let output_path = output_dir.join(&output_filename);

            let yt_dlp_cmd = format!(
                "{} {} -x --audio-format mp3 --audio-quality 0 -o {}",
                yt_path.display(),
                args.url,
                output_path.display()
            );

            println!("Executing custom yt-dlp command for MP3: {}", yt_dlp_cmd);

            let status = Command::new("cmd")
                .args(&["/C", &yt_dlp_cmd])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .context("Failed to execute yt-dlp command")?;

            if !status.success() {
                Err(anyhow!(
                    "yt-dlp command failed with exit code: {:?}",
                    status.code()
                ))
            } else {
                Ok(())
            }
        }
        "mp4" => {
            let output_path = output_dir.join(&output_filename);

            let yt_dlp_cmd = format!(
                "{} {} -f \"bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best\" --merge-output-format mp4 -o {}",
                yt_path.display(),
                args.url,
                output_path.display()
            );

            println!("Executing custom yt-dlp command for MP4: {}", yt_dlp_cmd);

            let status = Command::new("cmd")
                .args(&["/C", &yt_dlp_cmd])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .context("Failed to execute yt-dlp command")?;

            if !status.success() {
                Err(anyhow!(
                    "yt-dlp command failed with exit code: {:?}",
                    status.code()
                ))
            } else {
                Ok(())
            }
        }
        _ => unreachable!(),
    };

    // Finished progressbar
    progress_task.abort();
    pb.set_position(100);

    match result {
        Ok(_) => {
            pb.finish_with_message("Download completed successfully!");
            println!(
                "File saved to: {}",
                output_dir.join(&output_filename).display()
            );
            Ok(())
        }
        Err(e) => {
            pb.finish_with_message("Download failed!");
            Err(anyhow!("Failed to download: {}", e))
        }
    }
}

/// Extract zip file
fn extract_ffmpeg_zip(zip_path: &PathBuf, output_dir: &PathBuf) -> Result<()> {
    // Use Powershell command
    let expand_command = format!(
        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
        zip_path.display(),
        output_dir.display()
    );

    println!("Executing: {}", expand_command);

    let status = Command::new("powershell")
        .args(&["-Command", &expand_command])
        .status()
        .context("Failed to execute PowerShell to extract ffmpeg zip")?;

    if !status.success() {
        return Err(anyhow!(
            "Failed to extract ffmpeg zip. PowerShell exit code: {:?}",
            status.code()
        ));
    }

    Ok(())
}

/// Find ffmpeg.exe recursively in subdirectories
fn find_ffmpeg_in_dir(dir: &PathBuf) -> Result<PathBuf> {
    println!("Searching in: {}", dir.display());

    for entry in
        std::fs::read_dir(dir).context(format!("Failed to read directory: {}", dir.display()))?
    {
        if let Ok(entry) = entry {
            let path = entry.path();

            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    if filename == "ffmpeg.exe" {
                        println!("Found ffmpeg.exe at: {}", path.display());
                        return Ok(path);
                    }
                }
            } else if path.is_dir() {
                // Ignore massive and unrelated directories to avoid infinite loop
                // But maybe I think I don't need this because ffmpeg is an output
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
                if dir_name == "node_modules" || dir_name == ".git" {
                    continue;
                }

                // Recursion
                match find_ffmpeg_in_dir(&path) {
                    Ok(result) => return Ok(result),
                    Err(_) => continue,
                }
            }
        }
    }

    Err(anyhow!(
        "ffmpeg.exe not found in directory: {}",
        dir.display()
    ))
}

/// Remove ffmpeg related files
fn cleanup_ffmpeg_files(libs_dir: &PathBuf, zip_path: &PathBuf) -> Result<()> {
    println!("Cleaning up unnecessary FFmpeg files...");

    // Remove zip file
    if zip_path.exists() {
        std::fs::remove_file(zip_path)
            .context(format!("Failed to remove zip file: {}", zip_path.display()))?;
        println!("Removed: {}", zip_path.display());
    }

    // Remove other ffmpeg files
    for entry in std::fs::read_dir(libs_dir).context("Failed to read libs directory")? {
        if let Ok(entry) = entry {
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();

                if dir_name.contains("ffmpeg") || dir_name.contains("ffmpeg-7") {
                    println!("Removing directory: {}", path.display());
                    std::fs::remove_dir_all(&path)
                        .context(format!("Failed to remove directory: {}", path.display()))?;
                }
            }
        }
    }

    println!("Cleanup completed.");
    Ok(())
}

/// Remove cache dir
fn cleanup_cache_files() -> Result<()> {
    println!("Cleaning up cache directory");

    let cache_dir = &PathBuf::from("cache");
    if cache_dir.exists() {
        std::fs::remove_dir_all(cache_dir).context(format!(
            "Failed to remove cache directory: {}",
            cache_dir.display()
        ))?;
        println!("Removed: {}", cache_dir.display());
    }

    Ok(())
}

/// Download spinner animation on terminal
fn create_spinner_progress_bar(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {elapsed_precise} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

/// Download progress animation on terminal
fn create_download_progress_bar(message: &str) -> ProgressBar {
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}% ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(message.to_string());
    pb
}

/// Get timestampe
fn get_safe_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    timestamp.to_string()
}
