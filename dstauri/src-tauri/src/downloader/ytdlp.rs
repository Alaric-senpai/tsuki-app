//! yt-dlp process driver.
//!
//! Ported from `VideoDownloader.download_urls()` in the legacy
//! `downloader.py`, but driven through the CLI instead of the Python module:
//! the Rust side owns the process, streams machine-readable progress, honours
//! cancellation, and only reports success once the merged file is on disk with
//! a nonzero size.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::error::AppError;
use crate::media_ids;

use super::quality;

/// Prefix that marks a machine-readable progress line.
pub const PROGRESS_MARKER: &str = "TSKPROG|";

/// yt-dlp gets its own generous retry budget; the outer loop only retries once
/// yt-dlp has given up, so a slow connection is not reported as a failure.
const TRANSPORT_RETRIES: &str = "10";
const FRAGMENT_RETRIES: &str = "10";
const EXTRACTOR_RETRIES: &str = "3";
const FILE_ACCESS_RETRIES: &str = "3";
const SOCKET_TIMEOUT_SECS: &str = "30";

/// Keep at most this many console lines for error details.
const CONSOLE_TAIL: usize = 40;

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bps: f64,
    pub eta_sec: f64,
    pub format_id: String,
}

#[derive(Debug, Clone)]
pub enum Event {
    Progress(Progress),
    Console(String),
}

/// Everything one download attempt needs.
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub media_url: String,
    pub title: String,
    pub save_dir: PathBuf,
    pub quality: String,
    pub container: String,
    /// `mp4/mkv` lets yt-dlp pick a container that can hold the codecs.
    pub merge_output: String,
    pub browser: Option<String>,
    pub write_subs: bool,
    pub sub_langs: String,
    pub rate_limit_mb: f64,
}

/// Located yt-dlp binary.
pub struct YtDlp {
    program: PathBuf,
}

impl YtDlp {
    /// Prefer an updater-provided binary in app data, then `PATH`.
    pub fn locate(data_dir: &Path) -> Result<Self, AppError> {
        let name = format!("yt-dlp{}", std::env::consts::EXE_SUFFIX);
        let bundled = data_dir.join("bin").join(&name);
        if bundled.is_file() {
            return Ok(Self { program: bundled });
        }
        if let Some(found) = which(&name) {
            return Ok(Self { program: found });
        }
        Err(AppError::download_with(
            "yt-dlp was not found. Install it, or place the binary in the app's \
             data folder under bin/.",
            format!("looked for {name} in PATH and {}", bundled.display()),
        ))
    }

    /// Run one attempt. Returns the verified final media path.
    pub async fn run<F>(
        &self,
        options: &DownloadOptions,
        cancel: Arc<AtomicBool>,
        mut emit: F,
    ) -> Result<PathBuf, AppError>
    where
        F: FnMut(Event) + Send,
    {
        if cancel.load(Ordering::SeqCst) {
            return Err(cancelled());
        }
        fs::create_dir_all(&options.save_dir).map_err(|err| {
            AppError::io_with(
                "Could not create the download folder.",
                format!("{}: {err}", options.save_dir.display()),
            )
        })?;

        let print_file = unique_print_file(&options.save_dir);
        let args = build_args(options, &print_file);

        let mut child = Command::new(&self.program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| {
                AppError::download_with(
                    "Could not start yt-dlp.",
                    format!("{}: {err}", self.program.display()),
                )
            })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::download_with("Could not read yt-dlp's output.", "stdout was unavailable")
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AppError::download_with("Could not read yt-dlp's output.", "stderr was unavailable")
        })?;

        let (stderr_tx, mut stderr_rx) = mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if stderr_tx.send(line).is_err() {
                    break;
                }
            }
        });

        let mut console: Vec<String> = Vec::new();
        let mut stdout_lines = BufReader::new(stdout).lines();
        let mut stopped = false;

        loop {
            tokio::select! {
                line = stdout_lines.next_line() => {
                    match line {
                        Ok(Some(line)) => {
                            if let Some(progress) = parse_progress(&line) {
                                emit(Event::Progress(progress));
                            } else if !line.trim().is_empty() {
                                push_console(&mut console, &line);
                                emit(Event::Console(line));
                            }
                        }
                        Ok(None) => break,
                        Err(err) => {
                            push_console(&mut console, &format!("stdout error: {err}"));
                            break;
                        }
                    }
                }
                Some(line) = stderr_rx.recv() => {
                    push_console(&mut console, &line);
                    emit(Event::Console(line));
                }
                _ = tokio::time::sleep(Duration::from_millis(300)) => {
                    if cancel.load(Ordering::SeqCst) {
                        stopped = true;
                        let _ = child.start_kill();
                        break;
                    }
                }
            }
        }

        let status = child.wait().await;
        while let Ok(line) = stderr_rx.try_recv() {
            push_console(&mut console, &line);
        }

        let reported = read_print_file(&print_file);
        let _ = fs::remove_file(&print_file);

        if stopped || cancel.load(Ordering::SeqCst) {
            return Err(cancelled());
        }

        let exit_ok = status.map(|status| status.success()).unwrap_or(false);
        let detail = console.join("\n");

        let Some(final_path) = reported else {
            return Err(AppError::download_with(
                "yt-dlp finished without reporting an output file.",
                detail,
            ));
        };

        // A merge/post-processing failure leaves a file that does not exist or
        // is empty; only a real file counts as success.
        match fs::metadata(&final_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {}
            Ok(metadata) => {
                return Err(AppError::download_with(
                    "yt-dlp produced an empty output file.",
                    format!(
                        "{} ({} bytes)\n{detail}",
                        final_path.display(),
                        metadata.len()
                    ),
                ));
            }
            Err(err) => {
                return Err(AppError::download_with(
                    "yt-dlp did not leave a finished file on disk.",
                    format!("{}: {err}\n{detail}", final_path.display()),
                ));
            }
        }

        if !exit_ok {
            return Err(AppError::download_with(
                "yt-dlp reported an error after writing the file.",
                detail,
            ));
        }

        Ok(final_path)
    }
}

/// Build the CLI argument list for one attempt.
pub fn build_args(options: &DownloadOptions, print_file: &Path) -> Vec<OsString> {
    let (format, _) = quality::resolve_format(&options.quality);
    let mut args: Vec<OsString> = Vec::with_capacity(32);

    push(&mut args, "--newline");
    // Bundled yt-dlp is pinned; its periodic "older than 90 days" notice is
    // noise in the console (we have our own in-app updater).
    push(&mut args, "--no-warnings");
    push(&mut args, "--no-playlist");
    push(&mut args, "--no-simulate");
    push(&mut args, "--restrict-filenames");
    // Never ship a file with holes: a missing fragment aborts the attempt so
    // the outer retry loop can start over.
    push(&mut args, "--no-skip-unavailable-fragments");
    push(&mut args, "--retries");
    push(&mut args, TRANSPORT_RETRIES);
    push(&mut args, "--fragment-retries");
    push(&mut args, FRAGMENT_RETRIES);
    push(&mut args, "--extractor-retries");
    push(&mut args, EXTRACTOR_RETRIES);
    push(&mut args, "--file-access-retries");
    push(&mut args, FILE_ACCESS_RETRIES);
    push(&mut args, "--socket-timeout");
    push(&mut args, SOCKET_TIMEOUT_SECS);
    push(&mut args, "--format");
    push(&mut args, &format);
    push(&mut args, "--merge-output-format");
    if options.merge_output.trim().is_empty() {
        push(&mut args, quality::merge_container(&options.container));
    } else {
        push(&mut args, &options.merge_output);
    }
    push(&mut args, "--output");
    args.push(output_template(options));
    push(&mut args, "--progress-template");
    push(&mut args, &progress_template());
    push(&mut args, "--print-to-file");
    push(&mut args, "after_move:filepath");
    args.push(print_file.as_os_str().to_os_string());

    if options.write_subs {
        push(&mut args, "--write-subs");
        push(&mut args, "--write-auto-subs");
        push(&mut args, "--sub-langs");
        push(&mut args, &subtitle_langs(&options.sub_langs));
        push(&mut args, "--sub-format");
        push(&mut args, "srt");
    }

    if let Some(browser) = options
        .browser
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        push(&mut args, "--cookies-from-browser");
        push(&mut args, browser);
    }

    if options.rate_limit_mb > 0.0 {
        let bytes = (options.rate_limit_mb * 1024.0 * 1024.0).round() as u64;
        push(&mut args, "--limit-rate");
        push(&mut args, &bytes.to_string());
    }

    // The target URL is the last positional argument.
    args.push(OsString::from(&options.media_url));
    args
}

fn push(args: &mut Vec<OsString>, value: &str) {
    args.push(OsString::from(value));
}

/// `<save_dir>/<safe title>.%(ext)s`, with `%` escaped so a literal percent in
/// a title cannot start a yt-dlp format field.
pub fn output_template(options: &DownloadOptions) -> OsString {
    let clean = media_ids::safe_title(&options.title, 150);
    let path = if clean.is_empty() {
        options.save_dir.join("%(title)s.%(ext)s")
    } else {
        options
            .save_dir
            .join(format!("{}.%(ext)s", clean.replace('%', "%%")))
    };
    path.into_os_string()
}

fn progress_template() -> String {
    format!(
        "download:{PROGRESS_MARKER}\
         %(progress.downloaded_bytes)s|\
         %(progress.total_bytes)s|\
         %(progress.total_bytes_estimate)s|\
         %(progress.speed)s|\
         %(progress.eta)s|\
         %(info.format_id)s"
    )
}

fn subtitle_langs(raw: &str) -> String {
    let langs: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    if langs.is_empty() {
        "en".to_string()
    } else {
        langs.join(",")
    }
}

/// Parse one `--progress-template` line.
pub fn parse_progress(line: &str) -> Option<Progress> {
    let start = line.find(PROGRESS_MARKER)? + PROGRESS_MARKER.len();
    let payload = &line[start..];
    let mut parts = payload.split('|');
    let downloaded = number(parts.next()?).unwrap_or(0.0) as u64;
    let total = number(parts.next()?).unwrap_or(0.0) as u64;
    let estimate = number(parts.next()?).unwrap_or(0.0) as u64;
    let speed = number(parts.next()?).unwrap_or(0.0);
    let eta = number(parts.next()?).unwrap_or(0.0);
    let format_id = parts.next().unwrap_or("").trim().to_string();
    let total = total.max(estimate).max(downloaded);
    Some(Progress {
        downloaded_bytes: downloaded,
        total_bytes: total,
        speed_bps: speed,
        eta_sec: eta,
        format_id,
    })
}

/// yt-dlp renders missing template values as `NA`.
fn number(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("na") || trimmed == "None" {
        return None;
    }
    trimmed
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn push_console(console: &mut Vec<String>, line: &str) {
    console.push(line.to_string());
    if console.len() > CONSOLE_TAIL {
        console.remove(0);
    }
}

fn read_print_file(path: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(path).ok()?;
    text.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(PathBuf::from)
}

fn unique_print_file(save_dir: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    save_dir.join(format!(".tsuki-ytdlp-{}-{stamp}.path", std::process::id()))
}

fn cancelled() -> AppError {
    AppError::download("The download was cancelled.")
}

/// Minimal `PATH` lookup (no extra crate for one helper).
fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> DownloadOptions {
        DownloadOptions {
            media_url: "https://www.dailymotion.com/video/x8abc12".to_string(),
            title: "Xin Xia Episode 1".to_string(),
            save_dir: PathBuf::from("/tmp/tsuki-test"),
            quality: "720p".to_string(),
            container: "mp4".to_string(),
            merge_output: "mp4/mkv".to_string(),
            browser: None,
            write_subs: true,
            sub_langs: "en,es".to_string(),
            rate_limit_mb: 0.0,
        }
    }

    fn args_as_strings(options: &DownloadOptions) -> Vec<String> {
        build_args(options, Path::new("/tmp/report.path"))
            .into_iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn args_carry_the_legacy_retry_and_safety_settings() {
        let args = args_as_strings(&options());
        assert!(args.contains(&"--no-skip-unavailable-fragments".to_string()));
        assert!(args.contains(&"--restrict-filenames".to_string()));
        assert!(args.contains(&"--no-simulate".to_string()));
        assert!(args.contains(&"--socket-timeout".to_string()));
        assert!(args.contains(&"--fragment-retries".to_string()));
        assert_eq!(
            args[args.iter().position(|a| a == "--retries").expect("retries") + 1],
            "10"
        );
    }

    #[test]
    fn resolved_sources_merge_into_a_codec_friendly_container() {
        let args = args_as_strings(&options());
        let index = args
            .iter()
            .position(|a| a == "--merge-output-format")
            .expect("merge flag");
        assert_eq!(args[index + 1], "mp4/mkv");
    }

    #[test]
    fn subtitles_browser_and_rate_limit_are_optional() {
        let args = args_as_strings(&options());
        assert!(args.contains(&"--write-subs".to_string()));
        assert!(!args.contains(&"--cookies-from-browser".to_string()));
        assert!(!args.contains(&"--limit-rate".to_string()));

        let mut limited = options();
        limited.browser = Some("firefox".to_string());
        limited.rate_limit_mb = 2.0;
        let args = args_as_strings(&limited);
        assert!(args.contains(&"firefox".to_string()));
        let index = args
            .iter()
            .position(|a| a == "--limit-rate")
            .expect("rate flag");
        assert_eq!(args[index + 1], (2 * 1024 * 1024).to_string());
    }

    #[test]
    fn output_templates_escape_percent_signs() {
        let mut options = options();
        options.title = "100% Real".to_string();
        let template = output_template(&options).to_string_lossy().to_string();
        assert!(template.ends_with("100%% Real.%(ext)s"));
    }

    #[test]
    fn progress_lines_are_parsed_and_na_is_tolerated() {
        let line = "TSKPROG|1048576|4194304|NA|524288.0|6|137";
        let progress = parse_progress(line).expect("progress parses");
        assert_eq!(progress.downloaded_bytes, 1_048_576);
        assert_eq!(progress.total_bytes, 4_194_304);
        assert_eq!(progress.speed_bps, 524_288.0);
        assert_eq!(progress.eta_sec, 6.0);
        assert_eq!(progress.format_id, "137");

        let estimate = "TSKPROG|10|NA|100|NA|NA|";
        let progress = parse_progress(estimate).expect("estimate fallback");
        assert_eq!(progress.total_bytes, 100);

        assert!(parse_progress("plain log line").is_none());
    }

    #[test]
    fn subtitle_language_lists_are_cleaned() {
        assert_eq!(subtitle_langs(" en , es ,, "), "en,es");
        assert_eq!(subtitle_langs(""), "en");
    }
}
