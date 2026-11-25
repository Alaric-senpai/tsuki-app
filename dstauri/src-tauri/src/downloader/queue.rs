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

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use ts_rs::TS;

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
const ACTIVE_STATES: [&str; 5] = ["queued", "paused", "resolving", "downloading", "retrying"];

/// Maximum backoff between attempts (seconds).
const MAX_BACKOFF_SECS: f64 = 30.0;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct DownloadSnapshot {
    #[ts(as = "f64")]
    pub episode_id: i64,
    #[ts(as = "f64")]
    pub series_id: i64,
    pub title: String,
    /// `queued` | `paused` | `resolving` | `downloading` | `retrying` |
    /// `done` | `failed` | `cancelled`
    pub state: String,
    pub attempt: u32,
    pub max_retries: u32,
    #[ts(as = "f64")]
    pub downloaded_bytes: u64,
    #[ts(as = "f64")]
    pub total_bytes: u64,
    pub speed_bps: f64,
    pub eta_sec: f64,
    pub current_format: String,
    pub error: Option<String>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ConsoleLine {
    #[ts(as = "f64")]
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
    /// Put a job back at the head of the line (used when a running job is
    /// paused: it must not lose its place).
    EnqueueFront(Job),
    /// A worker finished (successfully or not) and released its slot.
    Finished,
    SetLimit(usize),
    PauseAll(bool),
    ResumeJob(i64),
    Reorder {
        episode_id: i64,
        /// Negative moves the job earlier, positive later.
        direction: i8,
    },
    Remove(i64),
}

type Snapshots = Arc<Mutex<HashMap<i64, DownloadSnapshot>>>;
type Cancels = Arc<Mutex<HashMap<i64, Arc<AtomicBool>>>>;
/// Episode ids the user paused. A paused job stays in the queue but is skipped
/// when slots free up; a running one is stopped and put back at the head.
type PausedJobs = Arc<Mutex<HashSet<i64>>>;
/// Ids the user removed from the queue. A worker that is still winding down
/// must not resurrect their rows, so every state update checks this set.
type Removed = Arc<Mutex<HashSet<i64>>>;

pub struct DownloadManager {
    commands: mpsc::UnboundedSender<Command>,
    snapshots: Snapshots,
    cancels: Cancels,
    paused_jobs: PausedJobs,
    removed: Removed,
    paused: Arc<AtomicBool>,
    /// Opt-in gate for completion notifications while the window is unfocused.
    background_notifications: Arc<AtomicBool>,
    app: AppHandle,
}

impl DownloadManager {
    pub fn new(db: SqlitePool, app: AppHandle, data_dir: PathBuf, concurrency: usize) -> Self {
        let (commands, receiver) = mpsc::unbounded_channel();
        let snapshots: Snapshots = Arc::new(Mutex::new(HashMap::new()));
        let cancels: Cancels = Arc::new(Mutex::new(HashMap::new()));
        let paused_jobs: PausedJobs = Arc::new(Mutex::new(HashSet::new()));
        let removed: Removed = Arc::new(Mutex::new(HashSet::new()));
        let paused = Arc::new(AtomicBool::new(false));
        let background_notifications = Arc::new(AtomicBool::new(false));

        let worker = Arc::new(Worker {
            background_notifications: Arc::clone(&background_notifications),
            db,
            app: app.clone(),
            data_dir,
            snapshots: Arc::clone(&snapshots),
            cancels: Arc::clone(&cancels),
            paused_jobs: Arc::clone(&paused_jobs),
            removed: Arc::clone(&removed),
            commands: commands.clone(),
        });
        let actor = QueueActor {
            worker,
            pending: VecDeque::new(),
            running: 0,
            limit: concurrency.clamp(1, 16),
            paused: false,
            paused_jobs: Arc::clone(&paused_jobs),
            commands: commands.clone(),
        };
        tauri::async_runtime::spawn(actor.run(receiver));

        Self {
            commands,
            snapshots,
            cancels,
            paused_jobs,
            removed,
            paused,
            background_notifications,
            app,
        }
    }

    /// Opt in (or out) of completion notifications for background downloads.
    pub fn set_background_notifications(&self, enabled: bool) {
        self.background_notifications
            .store(enabled, Ordering::SeqCst);
    }

    /// Change how many jobs may run at once.
    ///
    /// The actor owns admission: it starts pending jobs whenever a slot is free,
    /// so raising or lowering the limit only has to wake it up.
    pub fn set_concurrency(&self, limit: usize) {
        let _ = self.commands.send(Command::SetLimit(limit.clamp(1, 16)));
        refresh_tray(&self.app, &self.snapshots);
    }

    /// Hold the whole queue: running jobs keep going, nothing new starts.
    pub fn pause_all(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        let _ = self.commands.send(Command::PauseAll(paused));
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// True while any job is running or retrying.
    pub fn busy(&self) -> bool {
        self.snapshots
            .lock()
            .map(|map| {
                map.values().any(|snapshot| {
                    matches!(
                        snapshot.state.as_str(),
                        "resolving" | "downloading" | "retrying"
                    )
                })
            })
            .unwrap_or(false)
    }

    /// Pause one job. A queued job is skipped when slots free up; a running one
    /// is stopped and requeued at the head, keeping its partial files so yt-dlp
    /// continues instead of starting over.
    pub fn pause_job(&self, episode_id: i64) {
        if let Ok(mut set) = self.paused_jobs.lock() {
            set.insert(episode_id);
        }
        // Stopping the process is what makes a running job actually pause.
        self.cancel(episode_id);
        refresh_tray(&self.app, &self.snapshots);
    }

    /// Put a paused job back in line, ahead of everything still waiting.
    pub fn resume_job(&self, episode_id: i64) {
        if let Ok(mut set) = self.paused_jobs.lock() {
            set.remove(&episode_id);
        }
        let _ = self.commands.send(Command::ResumeJob(episode_id));
    }

    /// Move a waiting job earlier or later in the start order.
    pub fn reorder(&self, episode_id: i64, direction: i8) {
        let _ = self.commands.send(Command::Reorder {
            episode_id,
            direction,
        });
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
        if let Ok(mut set) = self.paused_jobs.lock() {
            set.remove(&episode_id);
        }
        let _ = self.commands.send(Command::Remove(episode_id));
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
        const ACTIVE: [&str; 5] = ["queued", "paused", "resolving", "downloading", "retrying"];

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
        if let Ok(mut set) = self.paused_jobs.lock() {
            set.retain(|episode_id| !removed.contains(episode_id));
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
    paused_jobs: PausedJobs,
    removed: Removed,
    /// Used to hand a paused job back to the queue.
    commands: mpsc::UnboundedSender<Command>,
    background_notifications: Arc<AtomicBool>,
}

/// Owns the waiting line.
///
/// Admission lives here rather than in a semaphore so the queue can be paused
/// and reordered: the actor decides which job starts next, and a worker tells
/// it when a slot frees up.
struct QueueActor {
    worker: Arc<Worker>,
    pending: VecDeque<Job>,
    running: usize,
    limit: usize,
    /// Whole-queue pause: running jobs continue, nothing new starts.
    paused: bool,
    paused_jobs: PausedJobs,
    commands: mpsc::UnboundedSender<Command>,
}

impl QueueActor {
    async fn run(mut self, mut receiver: mpsc::UnboundedReceiver<Command>) {
        while let Some(command) = receiver.recv().await {
            match command {
                Command::Enqueue(job) => {
                    self.pending.push_back(job);
                }
                Command::EnqueueFront(job) => {
                    self.pending.push_front(job);
                }
                Command::Finished => {
                    self.running = self.running.saturating_sub(1);
                }
                Command::SetLimit(limit) => {
                    self.limit = limit.clamp(1, 16);
                }
                Command::PauseAll(paused) => {
                    self.paused = paused;
                }
                Command::ResumeJob(episode_id) => {
                    if let Some(index) = self
                        .pending
                        .iter()
                        .position(|job| job.episode_id == episode_id)
                        && let Some(job) = self.pending.remove(index)
                    {
                        self.pending.push_front(job);
                    }
                }
                Command::Reorder {
                    episode_id,
                    direction,
                } => {
                    if let Some(index) = self
                        .pending
                        .iter()
                        .position(|job| job.episode_id == episode_id)
                    {
                        let target = move_index(self.pending.len(), index, direction);
                        if target != index {
                            self.pending.swap(index, target);
                        }
                    }
                }
                Command::Remove(episode_id) => {
                    self.pending.retain(|job| job.episode_id != episode_id);
                }
            }
            self.pump();
        }
    }

    /// Start as many pending jobs as the limit allows.
    fn pump(&mut self) {
        while !self.paused && self.running < self.limit {
            let Some(index) = self.next_startable() else {
                return;
            };
            let Some(job) = self.pending.remove(index) else {
                return;
            };
            self.running += 1;
            let worker = Arc::clone(&self.worker);
            let commands = self.commands.clone();
            tauri::async_runtime::spawn(async move {
                worker.run_job(job).await;
                let _ = commands.send(Command::Finished);
            });
        }
    }

    /// First waiting job the user has not paused.
    fn next_startable(&self) -> Option<usize> {
        let paused = self.paused_jobs.lock().ok()?;
        self.pending
            .iter()
            .position(|job| !paused.contains(&job.episode_id))
    }
}

impl Worker {
    async fn run_job(&self, job: Job) {
        let cancel = match self.cancels.lock() {
            Ok(mut map) => Arc::clone(
                map.entry(job.episode_id)
                    .or_insert_with(|| Arc::new(AtomicBool::new(false))),
            ),
            Err(_) => Arc::new(AtomicBool::new(false)),
        };

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
            self.stop_job(&job).await;
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
                self.stop_job(&job).await;
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
                        self.stop_job(&job).await;
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
                        self.stop_job(&job).await;
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
        if !self.background_notifications.load(Ordering::SeqCst) {
            return;
        }
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

    /// Cancel or pause a job that asked to stop.
    async fn stop_job(&self, job: &Job) {
        if self
            .paused_jobs
            .lock()
            .map(|set| set.contains(&job.episode_id))
            .unwrap_or(false)
        {
            self.pause_job(job).await;
        } else {
            self.cancel_job(job).await;
        }
    }

    /// Park a running job at the head of the queue. The process has already
    /// been stopped via the cancel flag; yt-dlp keeps its `.part` files, so the
    /// next run continues rather than starting over.
    async fn pause_job(&self, job: &Job) {
        let _ = queries::set_episode_status_by_id(&self.db, job.episode_id, "not_started").await;
        self.update(job, |snapshot| {
            snapshot.state = "paused".to_string();
            snapshot.error = None;
            snapshot.speed_bps = 0.0;
            snapshot.eta_sec = 0.0;
        });
        self.console(job, "paused".to_string());
        // A fresh cancel flag is needed when it resumes.
        self.release_cancel(job.episode_id);
        let _ = self.commands.send(Command::EnqueueFront(job.clone()));
        crate::events::library_changed(&self.app);
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

/// New index after moving a waiting job one step. Pure so ordering can be
/// unit-tested without an AppHandle.
fn move_index(len: usize, index: usize, direction: i8) -> usize {
    if len == 0 {
        return 0;
    }
    let index = index.min(len - 1);
    if direction < 0 {
        index.saturating_sub(1)
    } else {
        (index + 1).min(len - 1)
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
    fn move_index_clamps_at_both_ends() {
        assert_eq!(move_index(3, 0, -1), 0);
        assert_eq!(move_index(3, 1, -1), 0);
        assert_eq!(move_index(3, 1, 1), 2);
        assert_eq!(move_index(3, 2, 1), 2);
        assert_eq!(move_index(0, 7, 1), 0);
        assert_eq!(move_index(3, 99, 1), 2);
    }

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
