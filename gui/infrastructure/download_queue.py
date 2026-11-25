"""yt-dlp queue bridged into Qt signals — supports concurrent downloads."""
import re
import threading
import time
from typing import Any, Dict, List, Optional

from PyQt6.QtCore import QObject, QThread, pyqtSignal

from downloader import VideoDownloader

from ..utils import load_settings


_EXISTING_TAG = re.compile(r"^\[[^\]]+\]")


class QuietYdlLogger(QObject):
    """Routes yt-dlp console output into the Downloads terminal view."""

    line = pyqtSignal(str)
    tag = "[download]"

    def _emit(self, msg: Any, fallback: str) -> None:
        text = str(msg).strip()
        if not text:
            return
        if not _EXISTING_TAG.match(text):
            text = f"{fallback} {text}"
        self.line.emit(text)

    def debug(self, msg: Any):
        self._emit(msg, self.tag)

    def warning(self, msg: Any):
        self._emit(msg, "[warning]")

    def info(self, msg: Any):
        self._emit(msg, "[info]")

    def error(self, msg: Any):
        self._emit(msg, "[error]")


class DownloadTask:
    """One queued unit: a batch of URLs for one series."""

    def __init__(self, series_id: int, series_name: str, save_dir: str,
                 urls: List[str], quality: Optional[str] = None,
                 label: str = ""):
        self.series_id = series_id
        self.series_name = series_name
        self.save_dir = save_dir
        self.urls = urls
        self.quality = quality
        self.label = label or f"{series_name}"
        self.progress = 0.0
        self.status = "queued"
        # ---- live session telemetry (shown in the per-session detail window)
        self.step = "Queued"
        self.speed = 0.0
        self.peak_speed = 0.0
        self.downloaded = 0.0
        self.total = 0.0
        self.eta: Any = None
        self.current_file = ""
        self.error = ""
        self.logs: List[str] = []
        self.started_at: Optional[float] = None

    def __repr__(self):
        return f"<Task {self.series_name} {len(self.urls)} url(s) {self.status}>"


class DownloadWorker(QThread):
    """Executes one DownloadTask through the shared VideoDownloader engine."""

    # NOTE: never name a signal ``event`` on a QObject: it shadows
    # QObject.event() and Qt aborts with "native Qt signal is not callable".
    download_event = pyqtSignal(str, dict)
    line = pyqtSignal(str)

    def __init__(self, task: DownloadTask, settings: Dict[str, str], parent=None):
        super().__init__(parent)
        self.task = task
        self.settings = settings
        self._cancel_flag = threading.Event()

    def cancel(self):
        self._cancel_flag.set()

    def run(self):
        s = self.settings
        try:
            rate_mb = float(s.get("rate_limit_mb", "0") or 0)
        except (TypeError, ValueError):
            rate_mb = 0.0
        ydl_log = QuietYdlLogger()
        ydl_log.line.connect(self.line)  # type: ignore[arg-type]
        engine = VideoDownloader(
            quality=self.task.quality if self.task.quality else s.get("default_quality", "720p"),
            max_retries=int(s.get("max_retries", "3") or 3),
            browser=(s.get("browser_cookies", "brave") or "").strip().lower(),
            write_subs=True,
            on_event=lambda ev, data: self.download_event.emit(ev, dict(data or {})),
            cancel_check=self._cancel_flag.is_set,
            ydl_logger=ydl_log,
            container=s.get("default_container", "mp4") or "mp4",
            sub_langs=s.get("sub_langs", "en") or "en",
            rate_limit=(rate_mb if rate_mb > 0 else None),
        )
        try:
            summary = engine.download_urls(self.task.urls, self.task.save_dir,
                                           quality=self.task.quality)
            self.download_event.emit("summary", {
                "total": summary.total,
                "succeeded": list(summary.succeeded),
                "failed": [dict(f) for f in summary.failed],
                "skipped": list(summary.skipped),
            })
        except Exception as exc:  # keep the GUI alive if the engine explodes
            self.download_event.emit("summary", {"fatal": str(exc)})


class DownloadManager(QObject):
    """Parallel queue: up to `max_workers` tasks run simultaneously.

    "Pause All" freezes the queue (running tasks are allowed to finish);
    it does not abort active tasks.
    """

    state_changed = pyqtSignal()
    # task object, event name, event data
    download_event = pyqtSignal(object, str, dict)
    log_line = pyqtSignal(str)
    # task object, log line — per-session log stream
    task_log = pyqtSignal(object, str)
    task_done = pyqtSignal(object, bool)   # (task, ok)

    def __init__(self, parent: Optional[QObject] = None):
        super().__init__(parent)
        self.queue: List[DownloadTask] = []
        # Multiple concurrent workers (was single: active: Optional[DownloadWorker])
        self.active_workers: Dict[QThread, DownloadTask] = {}
        self.paused = False
        self.finished_log: List[Dict[str, Any]] = []
        self.settings: Dict[str, str] = load_settings()
        self.max_workers: int = max(1, int(self.settings.get("concurrent_downloads", "3") or 3))

    # ---- public API -------------------------------------------------------
    def enqueue(self, task: DownloadTask) -> None:
        self.settings = load_settings()
        self.max_workers = max(1, int(self.settings.get("concurrent_downloads", "3") or 3))
        self.queue.append(task)
        self.state_changed.emit()
        self._maybe_start_next()

    def remove(self, task: DownloadTask) -> None:
        if task in self.queue:
            self.queue.remove(task)
            self.state_changed.emit()
        # Also cancel if it's currently running
        for worker, t in list(self.active_workers.items()):
            if t is task:
                task.status = "cancelling"
                task.step = "Cancelling"
                self._task_log_line(task, "[status] Cancel requested — stopping yt-dlp…")
                worker.cancel()
        self.state_changed.emit()

    def move(self, task: DownloadTask, up: bool) -> None:
        if task not in self.queue:
            return
        i = self.queue.index(task)
        j = i - 1 if up else i + 1
        if 0 <= j < len(self.queue):
            self.queue[i], self.queue[j] = self.queue[j], self.queue[i]
            self.state_changed.emit()

    def clear_finished(self) -> None:
        self.finished_log.clear()
        self.state_changed.emit()

    def pause_all(self) -> None:
        self.paused = True
        self.state_changed.emit()

    def resume_all(self) -> None:
        self.paused = False
        self.state_changed.emit()
        self._maybe_start_next()

    def cancel_current(self) -> None:
        if self.active_workers:
            self.log_line.emit("[status] Cancel request sent to yt-dlp…")
            for worker in self.active_workers:
                worker.cancel()

    def shutdown(self, wait_ms: int = 3000) -> None:
        """Stop all workers before Qt tears down parent objects."""
        self.paused = True
        for worker in list(self.active_workers.keys()):
            worker.cancel()
        for worker in list(self.active_workers.keys()):
            if worker.wait(wait_ms // len(self.active_workers or [1])):
                continue
            worker.terminate()
            worker.wait(800)

    def busy(self) -> bool:
        return bool(self.active_workers)

    def set_max_workers(self, n: int) -> None:
        """Change concurrent download limit at runtime."""
        self.max_workers = max(1, n)
        self.state_changed.emit()
        self._maybe_start_next()

    # ---- internals ----------------------------------------------------------
    def _maybe_start_next(self) -> None:
        if self.paused:
            if self.queue:
                self.log_line.emit("[status] Queue paused — new tasks will wait.")
            return
        # Fill up to max_workers slots
        while len(self.active_workers) < self.max_workers and self.queue:
            task = self.queue.pop(0)
            task.status = "downloading"
            task.step = "Starting"
            task.started_at = time.time()
            start_line = (
                f"[status] Starting: {task.series_name} — {len(task.urls)} video(s)"
                + (f" ({task.label})" if task.label else "")
                + f"  ({len(self.active_workers)+1}/{self.max_workers} workers)"
            )
            self._task_log_line(task, start_line)
            self.log_line.emit(start_line)
            worker = DownloadWorker(task, self.settings, self)
            worker.download_event.connect(self._on_event)
            worker.line.connect(self.log_line)  # type: ignore[arg-type]
            worker.line.connect(
                lambda line, w=worker: self._on_worker_line(w, line))
            worker.finished.connect(self._on_worker_finished)
            self.active_workers[worker] = task
            worker.start()
        self.state_changed.emit()

    # ---- per-session log routing -------------------------------------------
    def _task_log_line(self, task: Optional[DownloadTask], line: str) -> None:
        if task is None:
            return
        text = str(line)
        task.logs.append(text)
        if len(task.logs) > 4000:
            del task.logs[:1500]
        self.task_log.emit(task, text)

    def _on_worker_line(self, worker: QThread, line: str) -> None:
        task = self.active_workers.get(worker)
        if task is not None:
            self._task_log_line(task, line)

    def _on_event(self, ev: str, data: dict) -> None:
        # Route progress to the right task
        task = None
        sender = self.sender()
        if sender and sender in self.active_workers:
            task = self.active_workers[sender]
        if task is not None:
            self._update_task_stats(task, ev, data)
        # Emit: task object, ev, data
        self.download_event.emit(task, ev, data)
        self.state_changed.emit()

    def _update_task_stats(self, task: DownloadTask, ev: str, data: dict) -> None:
        """Mirror engine telemetry onto the task for the detail window."""
        if ev == "start":
            task.status = "downloading"
            task.step = "Starting"
        elif ev == "progress":
            status = str(data.get("status") or "")
            if status in ("downloading", "stream_finished"):
                task.downloaded = float(data.get("downloaded_bytes") or 0)
                task.total = float(data.get("total_bytes") or 0)
                task.speed = float(data.get("speed") or 0)
                task.peak_speed = max(task.peak_speed, task.speed)
                task.eta = data.get("eta")
                task.step = ("Downloading" if status == "downloading"
                             else "Stream saved")
                name = str(data.get("filename") or "").strip()
                if name:
                    task.current_file = name
                if task.total > 0:
                    task.progress = task.downloaded / task.total
                    if status == "stream_finished":
                        task.progress = min(task.progress, 0.99)
            elif status == "finished":
                task.progress = 1.0
                task.step = "Merged"
        elif ev == "postprocess":
            if data.get("status") == "started":
                name = str(data.get("postprocessor") or "")
                task.step = "Merging streams" if "Merger" in name else "Finalizing"
        elif ev == "attempt":
            task.step = f"Attempt {data.get('attempt')}/{data.get('max_retries')}"
        elif ev == "retry_error":
            task.step = f"Retrying {data.get('attempt')}/{data.get('max_retries')}"
        elif ev == "failure":
            task.step = "Episode failed — continuing"
            task.error = str(data.get("error") or "")
        elif ev == "skip":
            task.step = "Skipped (already downloaded)"
        elif ev == "source":
            task.step = "Resolving source"
        elif ev == "summary":
            if "fatal" in data:
                task.step = "Failed"
                task.error = str(data.get("fatal") or "")
            else:
                failed = data.get("failed") or []
                task.step = "Completed with errors" if failed else "Completed"
                task.progress = 1.0

    def _on_worker_finished(self) -> None:
        sender = self.sender()
        if sender is None:
            return
        task = self.active_workers.pop(sender, None)
        if task is not None:
            task.status = "finished"
            task.progress = 1.0
            if not task.step.startswith(("Completed", "Failed", "Cancelled")):
                task.step = "Completed"
            self._task_log_line(
                task, f"[status] Session finished — {task.step}.")
            self.finished_log.append({
                "series": task.series_name, "dir": task.save_dir,
                "urls": list(task.urls), "label": task.label, "time": "just now",
            })
        self.state_changed.emit()
        self.task_done.emit(task, task is not None)
        self._maybe_start_next()

    def active_tasks(self) -> List[DownloadTask]:
        """Return list of all currently running tasks in order of start time."""
        return list(self.active_workers.values())

    def current_snapshot(self) -> Dict[str, Any]:
        """Snapshot of the first active task (for the now-playing card)."""
        tasks = self.active_tasks()
        if not tasks:
            return {}
        t = tasks[0]
        return {"task": t, "progress": t.progress, "label": t.label}
