use std::fs;
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

    // ディレクトリ構造の設定
    let libraries_dir = PathBuf::from("libs");
    let output_dir = args.output_dir.clone();
    
    // ディレクトリが存在することを確認
    fs::create_dir_all(&libraries_dir).context("Failed to create libraries directory")?;
    fs::create_dir_all(&output_dir).context("Failed to create output directory")?;
    
    println!("Initializing downloader...");
    
    // バイナリパスを設定（Windows環境を想定）
    let youtube_path = libraries_dir.join("yt-dlp.exe");
    let youtube_path_clone = youtube_path.clone();
    let ffmpeg_path = libraries_dir.join("ffmpeg.exe");
    
    // ffmpeg-release.zipが存在していてffmpeg.exeが存在しない場合は解凍
    let ffmpeg_zip_path = libraries_dir.join("ffmpeg-release.zip");
    
    // 必要なバイナリがあるか確認
    let mut yt_dlp_exists = youtube_path_clone.exists();
    let mut ffmpeg_exists = ffmpeg_path.exists();
    
    // 両方のバイナリが揃うまでループ
    while !yt_dlp_exists || !ffmpeg_exists {
        // yt-dlp.exeがない場合
        if !yt_dlp_exists {
            println!("yt-dlp.exe is missing. Trying to download...");
            
            // まず自動ダウンロードを試みる
            let pb = create_spinner_progress_bar("Downloading yt-dlp.exe");
            
            let temp_output_dir = output_dir.clone();
            match Youtube::with_new_binaries(libraries_dir.clone(), temp_output_dir).await {
                Ok(_) => {
                    pb.finish_with_message("yt-dlp.exe downloaded successfully");
                    if youtube_path_clone.exists() {
                        yt_dlp_exists = true;
                    } else {
                        pb.finish_with_message("Could not find yt-dlp.exe after download");
                        return Err(anyhow!("Could not download yt-dlp.exe"));
                    }
                },
                Err(e) => {
                    pb.finish_with_message("Failed to download yt-dlp.exe");
                    println!("Error: {}", e);
                    println!("Please download yt-dlp.exe manually and place it in the libs directory");
                    return Err(anyhow!("Failed to download yt-dlp.exe"));
                }
            }
        }
        
        // ffmpeg.exeがない場合
        if !ffmpeg_exists {
            // ffmpeg-release.zipがある場合は解凍
            if ffmpeg_zip_path.exists() {
                println!("Found ffmpeg zip file. Extracting...");
                extract_ffmpeg_zip(&ffmpeg_zip_path, &libraries_dir)?;
                
                // 抽出後にffmpeg.exeを探して移動
                println!("Searching for ffmpeg.exe in subdirectories...");
                match find_ffmpeg_in_dir(&libraries_dir) {
                    Ok(ffmpeg_path_found) => {
                        println!("Found ffmpeg.exe at: {}", ffmpeg_path_found.display());
                        fs::copy(&ffmpeg_path_found, &ffmpeg_path)
                            .context("Failed to copy ffmpeg.exe to libs directory")?;
                        println!("Copied ffmpeg.exe to: {}", ffmpeg_path.display());
                        
                        // 不要なディレクトリとzipファイルをクリーンアップ
                        cleanup_ffmpeg_files(&libraries_dir, &ffmpeg_zip_path)?;
                        
                        ffmpeg_exists = true;
                    },
                    Err(e) => {
                        println!("Error finding ffmpeg.exe: {}", e);
                        println!("Please download ffmpeg.exe manually and place it in the libs directory");
                        return Err(anyhow!("Could not find ffmpeg.exe after extraction"));
                    }
                }
            } else {
                println!("ffmpeg.exe and ffmpeg-release.zip are missing");
                println!("Please download ffmpeg.exe or ffmpeg-release.zip and place it in the libs directory");
                return Err(anyhow!("ffmpeg.exe is missing"));
            }
        }
    }
    
    println!("All required binaries are ready.");
    
    // フェッチャーの初期化（ここでのみ初期化）
    let libraries = Libraries::new(youtube_path_clone, ffmpeg_path);
    Youtube::new(libraries, output_dir.clone())
        .context("Failed to initialize Youtube fetcher")?;
    
    println!("Downloading from URL: {}", args.url);
    
    // 出力ファイル名を決定
    let output_filename = match args.format.as_str() {
        "mp3" => format!("audio_{}.mp3", get_safe_timestamp()),
        "mp4" => {
            let quality_str = match args.quality.as_str() {
                "best" => "best",
                "worst" => "worst",
                res => res,
            };
            format!("video_{}_{}.mp4", quality_str, get_safe_timestamp())
        },
        other => return Err(anyhow!("Unsupported format: {}", other)),
    };
    
    // プログレスバーを作成
    let pb = create_download_progress_bar(&format!("Downloading {} file", args.format));
    
    // 定期的にプログレスバーを更新するタスク
    let pb_clone = pb.clone();
    let progress_task = tokio::spawn(async move {
        for i in 1..=95 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            pb_clone.set_position(i);
        }
    });
    
    // ダウンロード実行
    let result = match args.format.as_str() {
        "mp3" => {
            // mp3ファイルのダウンロードをyt-dlp直接呼び出しに変更
            let output_path = output_dir.join(&output_filename);
            
            // yt-dlpコマンドを組み立て（mp3用）
            let yt_dlp_cmd = format!(
                "{} {} -x --audio-format mp3 --audio-quality 0 -o {}",
                youtube_path.display(),
                args.url,
                output_path.display()
            );
            
            println!("Executing custom yt-dlp command for MP3: {}", yt_dlp_cmd);
            
            // コマンドを実行
            let status = Command::new("cmd")
                .args(&["/C", &yt_dlp_cmd])
                .status()
                .context("Failed to execute yt-dlp command")?;
            
            if !status.success() {
                Err(anyhow!("yt-dlp command failed with exit code: {:?}", status.code()))
            } else {
                Ok(())
            }
        },
        "mp4" => {
            // Media Playerと互換性のあるフォーマットを指定するため、コマンドラインを直接実行
            let output_path = output_dir.join(&output_filename);
            
            // yt-dlpコマンドを組み立て（mp4用）
            let yt_dlp_cmd = format!(
                "{} {} -f \"bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best\" --merge-output-format mp4 -o {}",
                youtube_path.display(),
                args.url,
                output_path.display()
            );
            
            println!("Executing custom yt-dlp command for MP4: {}", yt_dlp_cmd);
            
            // コマンドを実行
            let status = Command::new("cmd")
                .args(&["/C", &yt_dlp_cmd])
                .status()
                .context("Failed to execute yt-dlp command")?;
            
            if !status.success() {
                Err(anyhow!("yt-dlp command failed with exit code: {:?}", status.code()))
            } else {
                Ok(())
            }
        },
        _ => unreachable!()
    };
    
    // プログレスバーを完了状態に設定
    progress_task.abort();
    pb.set_position(100);
    
    match result {
        Ok(_) => {
            pb.finish_with_message("Download completed successfully!");
            println!("File saved to: {}", output_dir.join(&output_filename).display());
            Ok(())
        },
        Err(e) => {
            pb.finish_with_message("Download failed!");
            Err(anyhow!("Failed to download: {}", e))
        }
    }
}

// ffmpeg zipファイルを解凍する関数
fn extract_ffmpeg_zip(zip_path: &PathBuf, output_dir: &PathBuf) -> Result<()> {
    // PowerShellを使用して解凍する（Windows環境向け）
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
        return Err(anyhow!("Failed to extract ffmpeg zip. PowerShell exit code: {:?}", status.code()));
    }
    
    Ok(())
}

// ディレクトリを再帰的に検索してffmpeg.exeを探して返す関数
fn find_ffmpeg_in_dir(dir: &PathBuf) -> Result<PathBuf> {
    println!("Searching in: {}", dir.display());
    
    for entry in fs::read_dir(dir).context(format!("Failed to read directory: {}", dir.display()))? {
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
                // 特定のディレクトリを無視（無限ループを避けるため）
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
                if dir_name == "node_modules" || dir_name == ".git" {
                    continue;
                }
                
                // サブディレクトリを再帰的に検索
                match find_ffmpeg_in_dir(&path) {
                    Ok(result) => return Ok(result),
                    Err(_) => continue,
                }
            }
        }
    }
    
    Err(anyhow!("ffmpeg.exe not found in directory: {}", dir.display()))
}

// 不要なffmpeg関連ファイルを削除する関数
fn cleanup_ffmpeg_files(libs_dir: &PathBuf, zip_path: &PathBuf) -> Result<()> {
    println!("Cleaning up unnecessary FFmpeg files...");
    
    // zipファイルを削除
    if zip_path.exists() {
        fs::remove_file(zip_path)
            .context(format!("Failed to remove zip file: {}", zip_path.display()))?;
        println!("Removed: {}", zip_path.display());
    }
    
    // 抽出されたffmpegディレクトリを探して削除
    for entry in fs::read_dir(libs_dir).context("Failed to read libs directory")? {
        if let Ok(entry) = entry {
            let path = entry.path();
            
            if path.is_dir() {
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
                // ffmpeg関連のディレクトリを削除（ffmpeg.exeを除く）
                if dir_name.contains("ffmpeg") || dir_name.contains("ffmpeg-7") {
                    println!("Removing directory: {}", path.display());
                    fs::remove_dir_all(&path)
                        .context(format!("Failed to remove directory: {}", path.display()))?;
                }
            }
        }
    }
    
    println!("Cleanup completed.");
    Ok(())
}

fn create_spinner_progress_bar(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {elapsed_precise} {msg}")
            .unwrap()
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

fn create_download_progress_bar(message: &str) -> ProgressBar {
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}% ({eta})")
            .unwrap()
            .progress_chars("#>-")
    );
    pb.set_message(message.to_string());
    pb
}

fn get_safe_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    timestamp.to_string()
}