"""Live per-session download window: progress, stats and the session log.

Each queued ``DownloadTask`` gets its own non-modal window so several
concurrent downloads can be watched independently.  The window mirrors the
task's live telemetry (speed, peak, transferred, ETA, current file, step)
and its private yt-dlp log stream.
"""
from __future__ import annotations

import os
from typing import Any, Dict, Optional

from PyQt6.QtCore import QSize, Qt, QTimer
from PyQt6.QtWidgets import (
    QApplication, QDialog, QFrame, QHBoxLayout, QLabel, QSizePolicy,
    QTextEdit, QVBoxLayout, QWidget,
)

from ..components.primitives import btn, label_caps, mk_label, styled_bar
from ..icons import icon
from ..infrastructure.download_queue import DownloadManager, DownloadTask
from ..theme import (
    T_AMBER2, T_AMBER_LINE, T_AMBER_SOFT, T_CANVAS_SOFT, T_CARD_SOFT,
    T_EMERALD2, T_EMERALD_LINE, T_EMERALD_SOFT, T_LINE, T_MUTED,
    T_PRIMARY, T_PRIMARY_LINE, T_PRIMARY_SOFT, T_ROSE2, T_ROSE_LINE,
    T_ROSE_SOFT, T_SEC, T_TEXT,
)
from ..utils import format_eta, human_size, speed_text

_TONES = {
    "default": (T_CARD_SOFT, T_LINE, T_SEC),
    "sky": (T_PRIMARY_SOFT, T_PRIMARY_LINE, T_PRIMARY),
    "emerald": (T_EMERALD_SOFT, T_EMERALD_LINE, T_EMERALD2),
    "amber": (T_AMBER_SOFT, T_AMBER_LINE, T_AMBER2),
    "rose": (T_ROSE_SOFT, T_ROSE_LINE, T_ROSE2),
}


class _StatTile(QFrame):
    """Monitor-style tile: caption, big value, sub value."""

    def __init__(self, caption: str, tone: str = "sky",
                 parent: Optional[QWidget] = None):
        super().__init__(parent)
        self.setObjectName("panel")
        self.setMinimumWidth(112)
        v = QVBoxLayout(self)
        v.setContentsMargins(12, 8, 12, 8)
        v.setSpacing(1)
        cap = QLabel(caption.upper())
        cap.setStyleSheet(
            f"color: {T_MUTED}; font-family: 'Fira Code'; font-size: 8px;")
        v.addWidget(cap)
        self.value = QLabel("—")
        color = (T_PRIMARY if tone == "sky" else
                 T_EMERALD2 if tone == "emerald" else T_AMBER2)
        self.value.setStyleSheet(
            f"color: {color}; font-size: 17px; font-weight: 700;"
            "font-family: 'Fira Code';")
        v.addWidget(self.value)
        self.sub = QLabel("")
        self.sub.setStyleSheet(f"color: {T_MUTED}; font-size: 9px;")
        v.addWidget(self.sub)

    def set(self, value: str, sub: str = "") -> None:
        self.value.setText(value)
        if sub:
            self.sub.setText(sub)


class DownloadDetailDialog(QDialog):
    """Non-modal live window for one download session."""

    def __init__(self, manager: DownloadManager, task: DownloadTask,
                 parent: Optional[QWidget] = None):
        super().__init__(parent)
        self.manager = manager
        self.task = task
        self._closed = False
        self._log_count = 0

        self.setWindowTitle(f"Download session — {task.series_name}")
        self.setMinimumSize(760, 560)
        self.resize(820, 640)
        self.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose, True)

        root = QVBoxLayout(self)
        root.setContentsMargins(20, 16, 20, 16)
        root.setSpacing(12)

        # ---- header -------------------------------------------------------
        head = QHBoxLayout()
        head.setSpacing(10)
        col = QVBoxLayout()
        col.setSpacing(2)
        col.addWidget(label_caps("download session"))
        col.addWidget(mk_label(task.series_name, "titleLg"))
        sub_bits = [f"{len(task.urls)} video(s)"]
        if task.label:
            sub_bits.append(task.label)
        if task.quality:
            sub_bits.append(str(task.quality))
        self.sub_lbl = mk_label(" · ".join(sub_bits), "small", "muted")
        col.addWidget(self.sub_lbl)
        head.addLayout(col, 1)
        self.status_chip = QLabel("QUEUED")
        head.addWidget(self.status_chip, 0, Qt.AlignmentFlag.AlignTop)
        root.addLayout(head)

        self.dir_lbl = QLabel(task.save_dir.replace(os.path.expanduser("~"), "~"))
        self.dir_lbl.setStyleSheet(
            f"color: {T_SEC}; font-family: 'Fira Code'; font-size: 10px;")
        self.dir_lbl.setToolTip(task.save_dir)
        self.dir_lbl.setSizePolicy(
            QSizePolicy.Policy.Ignored, QSizePolicy.Policy.Preferred)
        root.addWidget(self.dir_lbl)

        # ---- progress -----------------------------------------------------
        prog_card = QFrame()
        prog_card.setObjectName("card")
        pc = QVBoxLayout(prog_card)
        pc.setContentsMargins(16, 12, 16, 12)
        pc.setSpacing(8)

        step_row = QHBoxLayout()
        step_row.setSpacing(8)
        self.step_lbl = mk_label("Queued", "body")
        step_row.addWidget(self.step_lbl, 1)
        self.pct_lbl = mk_label("0%", "titleSm")
        step_row.addWidget(self.pct_lbl)
        pc.addLayout(step_row)

        self.bar = styled_bar("primary")
        self.bar.setFixedHeight(8)
        pc.addWidget(self.bar)

        self.file_lbl = mk_label("Waiting for the queue…", "small", "muted")
        self.file_lbl.setSizePolicy(
            QSizePolicy.Policy.Ignored, QSizePolicy.Policy.Preferred)
        pc.addWidget(self.file_lbl)

        tiles = QHBoxLayout()
        tiles.setSpacing(8)
        self.tile_rate = _StatTile("Rate")
        self.tile_peak = _StatTile("Peak")
        self.tile_xfer = _StatTile("Transferred", "emerald")
        self.tile_eta = _StatTile("ETA", "amber")
        for t in (self.tile_rate, self.tile_peak, self.tile_xfer, self.tile_eta):
            tiles.addWidget(t)
        pc.addLayout(tiles)
        root.addWidget(prog_card)

        # ---- session log ---------------------------------------------------
        log_card = QFrame()
        log_card.setObjectName("card")
        lc = QVBoxLayout(log_card)
        lc.setContentsMargins(12, 10, 12, 12)
        lc.setSpacing(8)
        lh = QHBoxLayout()
        lh.addWidget(label_caps("session log"))
        lh.addStretch(1)
        self.copy_btn = btn("Copy Log", "ghost", "Copy this session's log",
                            small=True)
        self.copy_btn.setFixedHeight(26)
        self.copy_btn.setIcon(icon("copy", T_SEC, 12))
        self.copy_btn.setIconSize(QSize(12, 12))
        self.copy_btn.clicked.connect(self._copy_log)
        lh.addWidget(self.copy_btn)
        lc.addLayout(lh)
        self.log = QTextEdit()
        self.log.setObjectName("console")
        self.log.setReadOnly(True)
        self.log.setMinimumHeight(200)
        self.log.setPlaceholderText("yt-dlp output for this session appears here…")
        self.log.setTextInteractionFlags(
            Qt.TextInteractionFlag.TextSelectableByMouse
            | Qt.TextInteractionFlag.TextSelectableByKeyboard)
        self.log.document().setMaximumBlockCount(3000)
        lc.addWidget(self.log, 1)
        root.addWidget(log_card, 1)

        # ---- actions -------------------------------------------------------
        actions = QHBoxLayout()
        actions.setSpacing(8)
        actions.addStretch(1)
        self.stop_btn = btn("Stop Session", "danger",
                            "Cancel this download only", small=True)
        self.stop_btn.setFixedHeight(32)
        self.stop_btn.setIcon(icon("close", T_ROSE2, 13))
        self.stop_btn.setIconSize(QSize(13, 13))
        self.stop_btn.clicked.connect(self._stop_session)
        actions.addWidget(self.stop_btn)
        close_btn = btn("Close", "ghost", "Close this window", small=True)
        close_btn.setFixedHeight(32)
        close_btn.clicked.connect(self.close)
        actions.addWidget(close_btn)
        root.addLayout(actions)

        # ---- live wiring ---------------------------------------------------
        self.manager.task_log.connect(self._on_task_log)
        self.manager.download_event.connect(self._on_event)
        self.manager.state_changed.connect(self._refresh)
        self._tick = QTimer(self)
        self._tick.setInterval(500)
        self._tick.timeout.connect(self._refresh)
        self._tick.start()

        for line in task.logs[-600:]:
            self._append(line)
        self._refresh()

    # ---- live updates ------------------------------------------------------
    def _set_chip(self, text: str, tone: str) -> None:
        bg, border, fg = _TONES.get(tone, _TONES["default"])
        self.status_chip.setText(text)
        self.status_chip.setStyleSheet(
            f"background: {bg}; border: 1px solid {border}; border-radius: 8px;"
            f" color: {fg}; font-family: 'Fira Code'; font-size: 10px;"
            " font-weight: 700; padding: 3px 9px;")

    def _refresh(self) -> None:
        if self._closed:
            return
        t = self.task
        pct = int(max(0.0, min(1.0, t.progress)) * 100)
        self.bar.setValue(pct)
        self.pct_lbl.setText(f"{pct}%")
        self.step_lbl.setText(str(t.step or "—"))
        self.tile_rate.set(speed_text(t.speed), "current speed")
        self.tile_peak.set(speed_text(t.peak_speed), "peak this session")
        self.tile_xfer.set(human_size(t.downloaded),
                           f"of {human_size(t.total)}" if t.total else "of unknown")
        self.tile_eta.set(format_eta(t.eta), "time remaining")

        name = t.current_file or ("Waiting for the queue…"
                                  if t.status == "queued" else "")
        self.file_lbl.setText(name)
        self.file_lbl.setToolTip(name)

        if t.status == "queued":
            self._set_chip("QUEUED", "default")
        elif t.status == "cancelling":
            self._set_chip("CANCELLING", "amber")
        elif t.status == "finished":
            if t.step.startswith("Failed"):
                self._set_chip("FAILED", "rose")
            elif "errors" in t.step:
                self._set_chip("COMPLETED WITH ERRORS", "amber")
            else:
                self._set_chip("COMPLETED", "emerald")
            self.bar.setValue(100)
        else:
            self._set_chip("● RUNNING", "sky")

        self.stop_btn.setEnabled(t.status in ("queued", "downloading",
                                              "cancelling"))
        if t.error:
            self.file_lbl.setToolTip(t.error)

    def _on_event(self, task: object, ev: str, data: Dict[str, Any]) -> None:
        if task is self.task:
            self._refresh()

    def _on_task_log(self, task: object, line: str) -> None:
        if task is self.task:
            self._append(line)

    def _append(self, line: str) -> None:
        from ..pages.downloads import terminal_html  # lazy: avoid import cycle
        text = str(line).replace("\r\n", "\n").replace("\r", "\n")
        for chunk in text.split("\n"):
            chunk = chunk.rstrip()
            if not chunk:
                continue
            self.log.append(terminal_html(chunk))
            self._log_count += 1
        self.log.ensureCursorVisible()

    # ---- actions -----------------------------------------------------------
    def _copy_log(self) -> None:
        text = self.log.toPlainText().strip() or "No log output yet."
        QApplication.clipboard().setText(text)
        self.copy_btn.setText("Copied")
        QTimer.singleShot(
            1600, lambda: self.copy_btn.setText("Copy Log")
            if not self._closed else None)

    def _stop_session(self) -> None:
        self.manager.remove(self.task)
        self._refresh()

    # ---- lifecycle ---------------------------------------------------------
    def closeEvent(self, event) -> None:  # noqa: N802 (Qt override)
        self._closed = True
        self._tick.stop()
        for sig, slot in (
            (self.manager.task_log, self._on_task_log),
            (self.manager.download_event, self._on_event),
            (self.manager.state_changed, self._refresh),
        ):
            try:
                sig.disconnect(slot)
            except (TypeError, RuntimeError):
                pass
        super().closeEvent(event)
