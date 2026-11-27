//! Internet connectivity tracking.
//!
//! A short, cached probe of a well-known `generate_204` endpoint decides
//! whether the app considers itself online. Probing runs on Tokio tasks, so it
//! never blocks startup or a command handler; the frontend is kept in sync by
//! the `network://…` events emitted from `events.rs`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use tauri::{AppHandle, Manager};

use crate::commands::downloads;
use crate::error::AppError;
use crate::events;
use crate::state::AppState;

const PRIMARY_ENDPOINT: &str = "https://www.google.com/generate_204";
const FALLBACK_ENDPOINT: &str = "https://clients3.google.com/generate_204";
const ENDPOINTS: [&str; 2] = [PRIMARY_ENDPOINT, FALLBACK_ENDPOINT];

/// Short enough that an offline machine does not wait; long enough for a weak
/// mobile connection to answer.
const PROBE_TIMEOUT: Duration = Duration::from_secs(4);
/// Repeated `is_online()` callers share a result for this long.
const CACHE_TTL: Duration = Duration::from_secs(15);
/// How often the background watcher re-probes.
pub const WATCHER_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct Sample {
    online: bool,
    checked_at: Instant,
}

static CACHE: OnceLock<RwLock<Option<Sample>>> = OnceLock::new();

fn cache() -> &'static RwLock<Option<Sample>> {
    CACHE.get_or_init(|| RwLock::new(None))
}

// A poisoned lock still holds a usable cache; recovering keeps a probe failure
// from turning into a panic in a background task.
fn read_cache() -> RwLockReadGuard<'static, Option<Sample>> {
    cache().read().unwrap_or_else(|err| err.into_inner())
}

fn write_cache() -> RwLockWriteGuard<'static, Option<Sample>> {
    cache().write().unwrap_or_else(|err| err.into_inner())
}

/// Last known state, regardless of how old it is.
fn last_known() -> Option<bool> {
    read_cache().as_ref().map(|sample| sample.online)
}

/// Last known state only while it is still fresh enough to reuse.
fn fresh() -> Option<bool> {
    read_cache()
        .as_ref()
        .and_then(|sample| (sample.checked_at.elapsed() < CACHE_TTL).then_some(sample.online))
}

fn record(online: bool) {
    *write_cache() = Some(Sample {
        online,
        checked_at: Instant::now(),
    });
}

fn probe_client() -> Result<&'static reqwest::Client, AppError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    let built = CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(PROBE_TIMEOUT)
            .user_agent(concat!(
                "Tsuki/",
                env!("CARGO_PKG_VERSION"),
                " (connectivity probe)"
            ))
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()
            .map_err(|err| err.to_string())
    });
    built
        .as_ref()
        .map_err(|err| AppError::io_with("Could not prepare the connectivity probe.", err.clone()))
}

/// Result of one full probe pass; `detail` is technical and safe to copy.
#[derive(Debug, Clone)]
pub struct Connectivity {
    pub online: bool,
    pub detail: String,
}

async fn probe(endpoint: &str) -> Result<(), String> {
    let client = probe_client().map_err(|err| err.detail().to_string())?;
    let response = client
        .get(endpoint)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    // `generate_204` must answer 204. A captive portal typically answers 200
    // with an HTML login page, which must not be mistaken for connectivity.
    if status == StatusCode::NO_CONTENT {
        Ok(())
    } else {
        Err(format!("unexpected HTTP {status}"))
    }
}

/// Hit the network now and update the cache. Always performs a real probe.
pub async fn check_now() -> Connectivity {
    let mut failures = Vec::with_capacity(ENDPOINTS.len());
    for endpoint in ENDPOINTS {
        match probe(endpoint).await {
            Ok(()) => {
                record(true);
                return Connectivity {
                    online: true,
                    detail: String::new(),
                };
            }
            Err(reason) => failures.push(format!("{endpoint}: {reason}")),
        }
    }
    record(false);
    Connectivity {
        online: false,
        detail: failures.join("; "),
    }
}

/// Cheap connectivity read: reuses a fresh sample, probes only when stale.
pub async fn is_online() -> bool {
    if let Some(online) = fresh() {
        return online;
    }
    check_now().await.online
}

/// Fail fast with a friendly, copyable error before any network-heavy work.
pub async fn ensure_online() -> Result<(), AppError> {
    let connectivity = match fresh() {
        Some(online) => Connectivity {
            online,
            detail: String::new(),
        },
        None => check_now().await,
    };
    if connectivity.online {
        return Ok(());
    }
    let detail = if connectivity.detail.is_empty() {
        "no connectivity endpoint answered with HTTP 204".to_string()
    } else {
        connectivity.detail
    };
    Err(AppError::io_with(
        "No internet connection. Reconnect and try again.",
        detail,
    ))
}

/// Frontend seed for the connectivity banner. The cache makes this cheap.
#[tauri::command]
pub async fn network_status() -> bool {
    is_online().await
}

/// Pure state-machine step, so transitions can be unit-tested without a network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// First sample: establish a baseline without notifying the UI.
    Initial(bool),
    Unchanged(bool),
    WentOffline,
    WentOnline,
}

pub fn transition(previous: Option<bool>, current: bool) -> Transition {
    match previous {
        None => Transition::Initial(current),
        Some(before) if before == current => Transition::Unchanged(current),
        Some(true) => Transition::WentOffline,
        Some(false) => Transition::WentOnline,
    }
}

static RESUME_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Requeue queued rows once connectivity returns. Only one resume runs at a
/// time; `resume_queued` re-checks the live queue, so running and paused jobs
/// are never restarted by this path.
fn resume_after_reconnect(app: &AppHandle) {
    if RESUME_IN_FLIGHT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(state) = handle.try_state::<AppState>() {
            match downloads::resume_queued(&state).await {
                Ok(count) if count > 0 => {
                    log::info!("resumed {count} queued download(s) after reconnecting")
                }
                Ok(_) => {}
                Err(error) => log::warn!(
                    "could not resume downloads after reconnecting: {}",
                    error.message()
                ),
            }
        }
        RESUME_IN_FLIGHT.store(false, Ordering::SeqCst);
    });
}

/// Background watcher: poll every ~30 s and emit only on actual changes.
pub fn spawn_watcher(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(WATCHER_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // `interval` ticks immediately; the startup probe already set the baseline.
        let _ = ticker.tick().await;

        loop {
            ticker.tick().await;
            let previous = last_known();
            let current = check_now().await.online;
            match transition(previous, current) {
                Transition::Unchanged(_) | Transition::Initial(_) => continue,
                Transition::WentOffline => {
                    events::network_offline(&app);
                    events::network_status(&app, false);
                }
                Transition::WentOnline => {
                    events::network_online(&app);
                    events::network_status(&app, true);
                    resume_after_reconnect(&app);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{Transition, transition};

    #[test]
    fn first_sample_establishes_a_baseline() {
        assert_eq!(transition(None, true), Transition::Initial(true));
        assert_eq!(transition(None, false), Transition::Initial(false));
    }

    #[test]
    fn repeated_samples_do_not_emit() {
        assert_eq!(transition(Some(true), true), Transition::Unchanged(true));
        assert_eq!(transition(Some(false), false), Transition::Unchanged(false));
    }

    #[test]
    fn changes_are_directional() {
        assert_eq!(transition(Some(true), false), Transition::WentOffline);
        assert_eq!(transition(Some(false), true), Transition::WentOnline);
    }
}
