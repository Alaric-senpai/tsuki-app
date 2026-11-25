//! Concurrent download queue.
//!
//! Ported from the retry/completion semantics of the legacy `downloader.py`:
//!
//! * a configurable number of jobs run at once (default 3), admitted in FIFO order;
//! * a failed attempt is retried with backoff, and the failure is reported
//!   only after the retry budget is exhausted;
//! * a job is complete only after yt-dlp merged every stream and the final
//!   file exists with a nonzero size — then the archive is reconciled;
//! * cancel stops the running process and leaves the episode untouched.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Semaphore, mpsc};

use crate::db::queries;
use crate::error::AppError;
use crate::media_ids;
use crate::sources;

use super::archive;
use super::ytdlp::{DownloadOptions, Event as YtEvent, YtDlp};

pub const EVENT_STATE: &str = "download://state";
pub const EVENT_CONSOLE: &str = "download://console";

/// Tray icon id, registered in `lib.rs`.
pub const TRAY_ID: &str = "downloads";

/// Snapshot states that count as an in-flight download.
const ACTIVE_STATES: [&str; 4] = ["queued", "resolving", "downloading", "retrying"];

/// Maximum backoff between attempts (seconds).
const MAX_BACKOFF_SECS: f64 = 30.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSnapshot {
    pub episode_id: i64,
    pub series_id: i64,
    pub title: String,
    /// `queued` | `resolving` | `downloading` | `retrying` | `done` |
    /// `failed` | `cancelled`
    pub state: String,
    pub attempt: u32,
    pub max_retries: u32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bps: f64,
    pub eta_sec: f64,
    pub current_format: String,
    pub error: Option<String>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleLine {
    pub episode_id: i64,
    pub line: String,
}

/// Everything one download needs, resolved at enqueue time so the worker does
/// not depend on the app configuration lock.
#[derive(Debug, Clone)]
pub struct Job {
    pub episode_id: i64,
    pub series_id: i64,
    pub video_id: String,
    pub page_url: String,
    pub title: String,
    pub save_dir: PathBuf,
    pub quality: String,
    pub container: String,
    pub browser: Option<String>,
    pub write_subs: bool,
    pub sub_langs: String,
    pub rate_limit_mb: f64,
    pub max_retries: u32,
}

enum Command {
    Enqueue(Job),
}

type Snapshots = Arc<Mutex<HashMap<i64, DownloadSnapshot>>>;
type Cancels = Arc<Mutex<HashMap<i64, Arc<AtomicBool>>>>;
/// Ids the user removed from the queue. A worker that is still winding down
/// must not resurrect their rows, so every state update checks this set.
type Removed = Arc<Mutex<HashSet<i64>>>;
/// Shared admission limit. A single semaphore is kept for the process
/// lifetime so changing concurrency can wake jobs already waiting on it.
type Limit = Arc<Semaphore>;

pub struct DownloadManager {
    commands: mpsc::UnboundedSender<Command>,
    snapshots: Snapshots,
    cancels: Cancels,
    removed: Removed,
    limit: Limit,
    configured: Arc<Mutex<usize>>,
    app: AppHandle,
}

impl DownloadManager {
    pub fn new(db: SqlitePool, app: AppHandle, data_dir: PathBuf, concurrency: usize) -> Self {
        let (commands, receiver) = mpsc::unbounded_channel();
        let snapshots: Snapshots = Arc::new(Mutex::new(HashMap::new()));
        let cancels: Cancels = Arc::new(Mutex::new(HashMap::new()));
        let removed: Removed = Arc::new(Mutex::new(HashSet::new()));
        let concurrency = concurrency.clamp(1, 16);
        let limit: Limit = Arc::new(Semaphore::new(concurrency));
        let configured: Arc<Mutex<usize>> = Arc::new(Mutex::new(concurrency));

        let worker = Arc::new(Worker {
            db,
            app: app.clone(),
            data_dir,
            snapshots: Arc::clone(&snapshots),
            cancels: Arc::clone(&cancels),
            removed: Arc::clone(&removed),
        });
        tauri::async_runtime::spawn(Arc::clone(&worker).run(receiver, Arc::clone(&limit)));

        Self {
            commands,
            snapshots,
            cancels,
            removed,
            limit,
            configured,
            app,
        }
    }

    /// Change how many jobs may run at once.
    ///
    /// One semaphore is kept for the lifetime of the manager: replacing it
    /// would strand jobs still waiting on the old instance. Running jobs keep
    /// their permits, so only the free permits are grown or shrunk.
    pub fn set_concurrency(&self, limit: usize) {
        let next = limit.clamp(1, 16);
        let current = self.configured.lock().map(|guard| *guard).unwrap_or(3);
        if current == next {
            return;
        }
        if let Ok(mut guard) = self.configured.lock() {
            *guard = next;
        }
        if next > current {
            self.limit.add_permits(next - current);
        } else {
            let removable = (current - next).min(self.limit.available_permits());
            if removable > 0 {
                self.limit.forget_permits(removable);
            }
        }
        refresh_tray(&self.app, &self.snapshots);
    }

    pub fn enqueue(&self, job: Job) -> Result<DownloadSnapshot, AppError> {
        let snapshot = DownloadSnapshot {
            episode_id: job.episode_id,
            series_id: job.series_id,
            title: job.title.clone(),
            state: "queued".to_string(),
            attempt: 0,
            max_retries: job.max_retries,
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_bps: 0.0,
            eta_sec: 0.0,
            current_format: String::new(),
            error: None,
            file_path: None,
        };
        if let Ok(mut map) = self.snapshots.lock() {
            map.insert(job.episode_id, snapshot.clone());
        }
        if let Ok(mut map) = self.cancels.lock() {
            map.insert(job.episode_id, Arc::new(AtomicBool::new(false)));
        }
        if let Ok(mut set) = self.removed.lock() {
            set.remove(&job.episode_id);
        }
        self.commands
            .send(Command::Enqueue(job))
            .map_err(|_| AppError::download("The download queue is not running."))?;
        crate::events::library_changed(&self.app);
        Ok(snapshot)
    }

    /// Ask the running job to stop. Safe to call for jobs that are not running.
    pub fn cancel(&self, episode_id: i64) {
        if let Ok(map) = self.cancels.lock()
            && let Some(flag) = map.get(&episode_id)
        {
            flag.store(true, Ordering::SeqCst);
        }
    }

    /// Stop a job (if it is still running) and drop its queue row.
    ///
    /// The id is remembered so a worker that is mid-retry cannot push the row
    /// back in; enqueueing the episode again clears that marker.
    pub fn remove(&self, episode_id: i64) -> bool {
        self.cancel(episode_id);
        if let Ok(mut set) = self.removed.lock() {
            set.insert(episode_id);
        }
        let known = self
            .snapshots
            .lock()
            .map(|mut map| map.remove(&episode_id).is_some())
            .unwrap_or(false);
        refresh_tray(&self.app, &self.snapshots);
        known
    }

    pub fn snapshot(&self, episode_id: i64) -> Option<DownloadSnapshot> {
        self.snapshots
            .lock()
            .ok()
            .and_then(|map| map.get(&episode_id).cloned())
    }

    /// Every known job, newest state first.
    pub fn snapshots(&self) -> Vec<DownloadSnapshot> {
        let mut rows: Vec<DownloadSnapshot> = self
            .snapshots
            .lock()
            .map(|map| map.values().cloned().collect())
            .unwrap_or_default();
        rows.sort_by_key(|row| row.episode_id);
        rows
    }

    /// Tell the frontend that catalog rows changed.
    pub fn notify_library_changed(&self) {
        crate::events::library_changed(&self.app);
    }

    /// Forget every finished job so the queue stops listing it.
    ///
    /// Only terminal states are dropped; queued and running jobs are left
    /// alone, and `update` can therefore never resurrect a row the user just
    /// cleared. Returns the episode ids that were removed.
    pub fn clear_finished(&self) -> Vec<i64> {
        const ACTIVE: [&str; 4] = ["queued", "resolving", "downloading", "retrying"];

        let mut removed: Vec<i64> = Vec::new();
        if let Ok(mut map) = self.snapshots.lock() {
            map.retain(|episode_id, snapshot| {
                if ACTIVE.contains(&snapshot.state.as_str()) {
                    true
                } else {
                    removed.push(*episode_id);
                    false
                }
            });
        }
        if let Ok(mut map) = self.cancels.lock() {
            for episode_id in &removed {
                map.remove(episode_id);
            }
        }

        removed.sort_unstable();
        if !removed.is_empty() {
            refresh_tray(&self.app, &self.snapshots);
        }
        removed
    }
}

/// Push the current queue depth and speed into the tray tooltip.
fn refresh_tray(app: &AppHandle, snapshots: &Snapshots) {
    let (count, speed_bps) = {
        let Ok(map) = snapshots.lock() else {
            return;
        };
        let mut count = 0usize;
        let mut speed = 0.0f64;
        for snapshot in map.values() {
            if !ACTIVE_STATES.contains(&snapshot.state.as_str()) {
                continue;
            }
            count += 1;
            speed += snapshot.speed_bps;
        }
        (count, speed)
    };

    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let tooltip = if count == 0 {
        "Tsuki — idle".to_string()
    } else {
        format!(
            "Tsuki — {count} downloading · {:.1} MB/s",
            speed_bps / (1024.0 * 1024.0)
        )
    };
    let _ = tray.set_tooltip(Some(tooltip));
    let _ = tray.set_title(if count == 0 {
        None
    } else {
        Some(count.to_string())
    });
}

struct Worker {
    db: SqlitePool,
    app: AppHandle,
    data_dir: PathBuf,
    snapshots: Snapshots,
    cancels: Cancels,
    removed: Removed,
}

impl Worker {
    /// Admit queued jobs in arrival order, up to the configured limit.
    async fn run(self: Arc<Self>, mut receiver: mpsc::UnboundedReceiver<Command>, limit: Limit) {
        while let Some(Command::Enqueue(job)) = receiver.recv().await {
            let worker = Arc::clone(&self);
            let permits = Arc::clone(&limit);
            tauri::async_runtime::spawn(async move {
                // `acquire_owned` wakes waiters in FIFO order, so the order the
                // user selected for a batch is the order jobs start.
                let Ok(_permit) = permits.acquire_owned().await else {
                    return;
                };
                worker.run_job(job).await;
            });
        }
    }

    async fn run_job(&self, job: Job) {
        let cancel = self
            .cancels
            .lock()
            .ok()
            .and_then(|map| map.get(&job.episode_id).cloned())
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

        self.update(&job, |snapshot| {
            snapshot.state = "resolving".to_string();
        });

        let resolved = match sources::resolve_for_download(&job.page_url).await {
            Ok(resolved) => resolved,
            Err(error) => {
                self.fail(&job, error.message(), Some(error.detail())).await;
                return;
            }
        };

        if cancel.load(Ordering::SeqCst) {
            self.cancel_job(&job).await;
            return;
        }

        let (media_url, resolved_title, media_source) = match resolved {
            Some(media) => {
                self.console(&job, format!("resolved {} → {}", job.page_url, media.url));
                (media.url, media.title, media.source)
            }
            None => (job.page_url.clone(), job.title.clone(), String::new()),
        };

        let options = DownloadOptions {
            media_url,
            title: if resolved_title.trim().is_empty() {
                job.title.clone()
            } else {
                resolved_title
            },
            save_dir: job.save_dir.clone(),
            quality: job.quality.clone(),
            container: job.container.clone(),
            // Resolved streams pick their own container: forcing the user's
            // choice breaks real downloads (e.g. AV1 + AAC cannot mux to webm).
            merge_output: if media_source.is_empty() {
                super::quality::merge_container(&job.container).to_string()
            } else {
                "mp4/mkv".to_string()
            },
            browser: job.browser.clone(),
            write_subs: job.write_subs,
            sub_langs: job.sub_langs.clone(),
            rate_limit_mb: job.rate_limit_mb,
        };

        let max_attempts = job.max_retries.max(1);
        let ytdlp = match YtDlp::locate(&self.data_dir) {
            Ok(ytdlp) => ytdlp,
            Err(error) => {
                self.fail(&job, error.message(), Some(error.detail())).await;
                return;
            }
        };

        let mut last_error = String::new();
        let mut last_detail = String::new();

        for attempt in 1..=max_attempts {
            if cancel.load(Ordering::SeqCst) {
                self.cancel_job(&job).await;
                return;
            }

            self.update(&job, |snapshot| {
                snapshot.state = "downloading".to_string();
                snapshot.attempt = attempt;
                snapshot.error = None;
                snapshot.downloaded_bytes = 0;
                snapshot.total_bytes = 0;
                snapshot.speed_bps = 0.0;
                snapshot.eta_sec = 0.0;
            });

            let app = self.app.clone();
            let snapshots = Arc::clone(&self.snapshots);
            let episode_id = job.episode_id;
            let mut emit = move |event: YtEvent| match event {
                YtEvent::Progress(progress) => {
                    let payload = {
                        let Ok(mut map) = snapshots.lock() else {
                            return;
                        };
                        let Some(snapshot) = map.get_mut(&episode_id) else {
                            return;
                        };
                        snapshot.downloaded_bytes = progress.downloaded_bytes;
                        snapshot.total_bytes = progress.total_bytes;
                        snapshot.speed_bps = progress.speed_bps;
                        snapshot.eta_sec = progress.eta_sec;
                        snapshot.current_format = progress.format_id.clone();
                        snapshot.clone()
                    };
                    let _ = app.emit(EVENT_STATE, payload);
                }
                YtEvent::Console(line) => {
                    let payload = ConsoleLine { episode_id, line };
                    let _ = app.emit(EVENT_CONSOLE, payload);
                }
            };

            let result = ytdlp.run(&options, Arc::clone(&cancel), &mut emit).await;

            match result {
                Ok(path) => {
                    self.complete(&job, &path).await;
                    return;
                }
                Err(error) => {
                    if cancel.load(Ordering::SeqCst) {
                        self.cancel_job(&job).await;
                        return;
                    }
                    last_error = error.message().to_string();
                    last_detail = error.detail().to_string();

                    // Some URLs (e.g. generic embeds we have no resolver for)
                    // can never succeed — retrying them only floods the console.
                    // Fail once, immediately, and let the UI report it.
                    if last_detail.contains("Unsupported URL")
                        || last_error.contains("Unsupported URL")
                    {
                        break;
                    }

                    // A merge failure leaves unusable streams behind; purge
                    // them so the next attempt fetches fresh instead of
                    // re-merging the same corrupt file forever.
                    if is_postprocess_error(&last_detail)
                        && let Ok(removed) = purge_intermediates(&job.save_dir, &options.title)
                        && removed > 0
                    {
                        self.console(
                            &job,
                            format!("purged {removed} intermediate file(s) after a merge failure"),
                        );
                    }

                    if attempt >= max_attempts {
                        break;
                    }

                    let backoff =
                        Duration::from_secs_f64((2.0 * f64::from(attempt)).min(MAX_BACKOFF_SECS));
                    self.update(&job, |snapshot| {
                        snapshot.state = "retrying".to_string();
                        snapshot.error = Some(last_error.clone());
                    });
                    self.console(
                        &job,
                        format!(
                            "attempt {attempt}/{max_attempts} failed: {last_error} — retrying in {}s",
                            backoff.as_secs()
                        ),
                    );
                    if wait_or_cancel(backoff, &cancel).await {
                        self.cancel_job(&job).await;
                        return;
                    }
                }
            }
        }

        self.fail(&job, &last_error, Some(&last_detail)).await;
    }

    async fn complete(&self, job: &Job, path: &Path) {
        if let Err(error) = archive::add_archive_id(&job.save_dir.to_string_lossy(), &job.video_id)
        {
            // The media exists, so the episode is downloaded; only the archive
            // bookkeeping failed. Surface it in the console, not as a failure.
            self.console(job, format!("could not update the archive: {error}"));
        }
        if let Err(error) =
            queries::set_episode_status_by_id(&self.db, job.episode_id, "downloaded").await
        {
            self.console(job, format!("could not update the episode row: {error}"));
        }

        let file_path = path.to_string_lossy().to_string();
        // Persist the path so /watch still works after a restart or after the
        // in-memory queue row is cleared.
        if let Err(error) =
            queries::set_episode_file_path(&self.db, job.episode_id, &file_path).await
        {
            self.console(job, format!("could not remember the file path: {error}"));
        }
        self.update(job, |snapshot| {
            snapshot.state = "done".to_string();
            snapshot.error = None;
            snapshot.file_path = Some(file_path.clone());
            if snapshot.total_bytes == 0 {
                snapshot.total_bytes = snapshot.downloaded_bytes;
            }
        });
        self.console(job, format!("saved {}", path.display()));
        self.notify_complete(job);
        // Free the queue row so the next waiting job visibly takes this slot.
        if let Ok(mut map) = self.snapshots.lock() {
            map.remove(&job.episode_id);
        }
        refresh_tray(&self.app, &self.snapshots);
        crate::events::library_changed(&self.app);
        self.release_cancel(job.episode_id);
    }

    /// Desktop notification so a background download is not silent.
    fn notify_complete(&self, job: &Job) {
        use tauri_plugin_notification::NotificationExt;
        if let Err(error) = self
            .app
            .notification()
            .builder()
            .title("Download complete")
            .body(&job.title)
            .show()
        {
            self.console(job, format!("could not show a notification: {error}"));
        }
    }

    async fn fail(&self, job: &Job, message: &str, detail: Option<&str>) {
        // Failures are reported only after the retry budget is exhausted.
        let _ = queries::set_episode_status_by_id(&self.db, job.episode_id, "failed").await;
        self.update(job, |snapshot| {
            snapshot.state = "failed".to_string();
            snapshot.error = Some(message.to_string());
        });
        if let Some(detail) = detail.filter(|value| !value.is_empty()) {
            self.console(job, detail.to_string());
        }
        self.console(job, format!("failed: {message}"));
        crate::events::library_changed(&self.app);
        self.release_cancel(job.episode_id);
    }

    async fn cancel_job(&self, job: &Job) {
        let _ = queries::set_episode_status_by_id(&self.db, job.episode_id, "not_started").await;
        self.update(job, |snapshot| {
            snapshot.state = "cancelled".to_string();
            snapshot.error = None;
            snapshot.speed_bps = 0.0;
            snapshot.eta_sec = 0.0;
        });
        self.console(job, "cancelled".to_string());
        crate::events::library_changed(&self.app);
        self.release_cancel(job.episode_id);
    }

    fn update<F>(&self, job: &Job, change: F)
    where
        F: FnOnce(&mut DownloadSnapshot),
    {
        // A row the user removed must stay removed, even if this worker is
        // still emitting progress while it shuts down.
        if self
            .removed
            .lock()
            .map(|set| set.contains(&job.episode_id))
            .unwrap_or(false)
        {
            return;
        }

        let payload = {
            let Ok(mut map) = self.snapshots.lock() else {
                return;
            };
            let snapshot = map
                .entry(job.episode_id)
                .or_insert_with(|| DownloadSnapshot {
                    episode_id: job.episode_id,
                    series_id: job.series_id,
                    title: job.title.clone(),
                    state: "queued".to_string(),
                    attempt: 0,
                    max_retries: job.max_retries,
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    speed_bps: 0.0,
                    eta_sec: 0.0,
                    current_format: String::new(),
                    error: None,
                    file_path: None,
                });
            change(snapshot);
            snapshot.clone()
        };
        let _ = self.app.emit(EVENT_STATE, payload);
        self.refresh_tray();
    }

    fn refresh_tray(&self) {
        refresh_tray(&self.app, &self.snapshots);
    }

    fn console(&self, job: &Job, line: String) {
        let _ = self.app.emit(
            EVENT_CONSOLE,
            ConsoleLine {
                episode_id: job.episode_id,
                line,
            },
        );
    }

    fn release_cancel(&self, episode_id: i64) {
        if let Ok(mut map) = self.cancels.lock() {
            map.remove(&episode_id);
        }
    }
}

/// Cancellation-aware backoff. Returns `true` when cancellation was requested.
async fn wait_or_cancel(duration: Duration, cancel: &AtomicBool) -> bool {
    let deadline = Instant::now() + duration;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return true;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        tokio::time::sleep(remaining.min(Duration::from_millis(200))).await;
    }
}

/// True when an error means the downloaded streams could not be merged.
pub fn is_postprocess_error(message: &str) -> bool {
    let text = message.to_lowercase();
    text.contains("postprocessing")
        || text.contains("conversion failed")
        || text.contains("invalid data found when processing input")
}

/// Delete an episode's in-progress download files after a merge failure.
pub fn purge_intermediates(save_dir: &Path, title: &str) -> std::io::Result<usize> {
    let prefix = media_ids::safe_title(title, 150);
    if prefix.is_empty() {
        return Ok(0);
    }
    let mut removed = 0;
    for entry in std::fs::read_dir(save_dir)?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !name.starts_with(&prefix) || !media_ids::is_intermediate_media_name(name) {
            continue;
        }
        if std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postprocess_errors_are_recognised() {
        assert!(is_postprocess_error(
            "ERROR: Postprocessing: Conversion failed!"
        ));
        assert!(is_postprocess_error(
            "Invalid data found when processing input"
        ));
        assert!(!is_postprocess_error("HTTP Error 404: Not Found"));
    }

    #[test]
    fn intermediates_are_purged_but_finished_files_survive() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        std::fs::write(root.join("Episode 1.mp4"), b"final").expect("final file");
        std::fs::write(root.join("Episode 1.f137.mp4"), b"stream").expect("stream file");
        std::fs::write(root.join("Episode 1.mp4.part"), b"partial").expect("part file");

        let removed = purge_intermediates(root, "Episode 1").expect("purge");
        assert_eq!(removed, 2);
        assert!(root.join("Episode 1.mp4").exists());
        assert!(!root.join("Episode 1.f137.mp4").exists());
        assert!(!root.join("Episode 1.mp4.part").exists());
    }
}
