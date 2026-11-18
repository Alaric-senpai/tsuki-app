"""Minimal in-app video player panel with streaming support.

Appears as an overlay panel inside the Explore view (or any page that
needs it).  Uses QtMultimedia + QVideoWidget for playback and runs the
yt-dlp URL-extraction in a background thread so the UI never freezes.
"""
from __future__ import annotations

import subprocess
import threading
import time
from typing import Optional

from PyQt6.QtCore import (
    QLibraryInfo, QSize, Qt, QThread, QUrl, pyqtSignal, pyqtSlot,
)
from PyQt6.QtGui import QDesktopServices, QKeySequence
from PyQt6.QtMultimedia import QAudioOutput, QMediaPlayer
from PyQt6.QtMultimediaWidgets import QVideoWidget
from PyQt6.QtWidgets import (
    QApplication, QHBoxLayout, QLabel, QPushButton, QSizePolicy,
    QSlider, QVBoxLayout, QWidget,
)

import sources
from ..icons import icon
from ..theme import (
    T_CANVAS, T_LINE, T_MUTED, T_PRIMARY, T_ROSE2, T_SEC, T_TEXT,
)


# ---- yt-dlp stream extractor thread -----------------------------------------

class _StreamExtractor(QThread):
    """Resolve a video page URL to a direct stream URL using yt-dlp."""

    stream_ready = pyqtSignal(str)          # resolved stream URL
    error = pyqtSignal(str)                # error message

    def __init__(self, video_url: str, parent: QWidget | None = None):
        super().__init__(parent)
        self._video_url = video_url
        self._stopped = False

    def run(self) -> None:
        try:
            proc = subprocess.run(
                [
                    "yt-dlp",
                    "--no-warnings",
                    "--no-playlist",
                    "--dump-json",
                    "--flat-playlist",
                    "-f", "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best",
                    "--get-url",
                    str(self._video_url),
                ],
                capture_output=True,
                text=True,
                timeout=60,
            )
            if self._stopped:
                return
            if proc.returncode == 0:
                url = proc.stdout.strip()
                if url:
                    self.stream_ready.emit(url)
                    return
            err = proc.stderr.strip() or "yt-dlp returned no stream URL"
            self.error.emit(err)
        except subprocess.TimeoutExpired:
            self.error.emit("Stream extraction timed out (60 s).")
        except FileNotFoundError:
            self.error.emit("yt-dlp is not installed.")
        except Exception as exc:
            self.error.emit(str(exc))

    def stop(self) -> None:
        self._stopped = True
        self.quit()
        self.wait(3000)


# ---- player panel -----------------------------------------------------------

def _player_style() -> str:
    return f"""
        QWidget#playerRoot {{
            background: {T_CANVAS};
            border-top: 1px solid {T_LINE};
        }}
        QLabel#titleLabel {{
            color: {T_TEXT};
            font-size: 14px;
            font-weight: 700;
        }}
        QLabel#statusLabel {{
            color: {T_SEC};
            font-size: 11px;
        }}
        QPushButton {{
            background: {T_CARD_SOFT};
            color: {T_TEXT};
            border: 1px solid {T_LINE};
            border-radius: 6px;
            padding: 6px 14px;
            font-size: 12px;
            font-weight: 600;
        }}
        QPushButton:hover {{ background: {T_HOVER}; }}
        QPushButton:pressed {{ background: {T_CARD_HI}; }}
        QPushButton#iconBtn {{
            background: transparent;
            border: none;
            border-radius: 20px;
            padding: 6px;
        }}
        QPushButton#iconBtn:hover {{ background: #2f2f2f; }}
        QSlider::groove:horizontal {{
            border: none;
            height: 4px;
            background: {T_LINE};
            border-radius: 2px;
        }}
        QSlider::handle:horizontal {{
            background: {T_PRIMARY};
            width: 12px;
            margin: -4px 0;
            border-radius: 6px;
        }}
        QSlider::sub-page:horizontal {{
            background: {T_PRIMARY};
            border-radius: 2px;
        }}
    """


class VideoPlayerPanel(QWidget):
    """Overlay panel that appears above the explore grid to play a video stream.

    Supports three sources:
      1. A direct stream URL  (YouTube / Dailymotion HLS via yt-dlp extraction)
      2. A local file path   (already-downloaded episodes — system player)
      3. A web page URL      (fallback: open in default browser)

    The panel has a thin media-control strip at the bottom and hides itself
    when closed, returning focus to the explore grid.
    """

    # Emitted when the user asks to open the current video in an external browser.
    open_in_browser = pyqtSignal(str)

    closed = pyqtSignal()   # user closed the panel

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        self.setObjectName("playerRoot")
        self.setStyleSheet(_player_style())
        self.setFixedHeight(0)          # hidden by default; slide in when opened
        self.setSizePolicy(QSizePolicy.Policy.Expanding,
                           QSizePolicy.Policy.Fixed)

        self._video_url = ""            # page URL passed in
        self._stream_url = ""           # resolved stream URL (or local file)
        self._title = ""
        self._extractor: Optional[_StreamExtractor] = None
        self._player: Optional[QMediaPlayer] = None
        self._audio: Optional[QAudioOutput] = None
        self._video_widget: Optional[QVideoWidget] = None
        self._opened = False            # is the panel currently showing content?

        self._build_ui()

    # ---- public API -----------------------------------------------------------

    def play_url(self, video_url: str, title: str = "") -> None:
        """Show the panel and play the given video page URL or local file path."""
        self._video_url = str(video_url or "").strip()
        self._title = str(title or "")
        self._stream_url = ""
        self._cancel_extractor()

        if not self._video_url:
            return

        # Local file → play directly
        if self._video_url.startswith("/") or self._video_url.startswith("file://"):
            path = self._video_url.removeprefix("file://")
            self._stream_url = QUrl.fromLocalFile(path).toString()
            self._show_streaming()
            return

        self._title_lbl.setText(self._title or "Loading stream…")
        self._status_lbl.setText("Extracting stream URL with yt-dlp…")
        self._controls.hide()
        self._error_lbl.hide()
        self._slide_in()
        self._opened = True
        self._start_extractor(self._video_url)

    def play_stream(self, stream_url: str, title: str = "") -> None:
        """Play an already-resolved direct stream URL (bypass yt-dlp)."""
        self._video_url = ""
        self._stream_url = str(stream_url or "").strip()
        self._title = str(title or "")
        self._title_lbl.setText(self._title or "Stream")
        self._status_lbl.setText("")
        self._controls.hide()
        self._error_lbl.hide()
        self._slide_in()
        self._opened = True
        self._init_player(QUrl(self._stream_url))

    def close_panel(self) -> None:
        """Stop playback and hide the panel."""
        self._cancel_extractor()
        self._stop_player()
        self._slide_out()
        self._opened = False
        self.closed.emit()

    # ---- UI construction ------------------------------------------------------

    def _build_ui(self) -> None:
        lay = QVBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.setSpacing(0)

        # ── video area ───────────────────────────────────────────────────────
        self._video_container = QWidget()
        self._video_container.setFixedHeight(480)
        self._video_container.setStyleSheet(
            f"background: {T_CANVAS}; border-bottom: 1px solid {T_LINE};")
        vlay = QVBoxLayout(self._video_container)
        vlay.setContentsMargins(0, 0, 0, 0)
        vlay.setSpacing(0)

        self._placeholder = QLabel("▶")
        self._placeholder.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._placeholder.setStyleSheet(
            f"color: {T_MUTED}; font-size: 48px; background: {T_CANVAS};")
        self._placeholder.setFixedHeight(480)
        vlay.addWidget(self._placeholder)

        # QtMultimedia video widget (added on first stream only)
        self._vid_widget: Optional[QVideoWidget] = None

        # ── controls strip ───────────────────────────────────────────────────
        self._controls = QWidget()
        self._controls.setObjectName("controls")
        self._controls.setFixedHeight(52)
        self._controls.setStyleSheet(
            f"background: {T_CANVAS_SOFT}; border-bottom: 1px solid {T_LINE};")
        self._controls.setSizePolicy(QSizePolicy.Policy.Expanding,
                                      QSizePolicy.Policy.Fixed)
        cl = QVBoxLayout(self._controls)
        cl.setContentsMargins(16, 6, 16, 6)
        cl.setSpacing(4)

        # title + error row
        top = QHBoxLayout()
        top.setSpacing(8)
        self._title_lbl = QLabel("")
        self._title_lbl.setObjectName("titleLabel")
        self._title_lbl.setStyleSheet(
            f"color: {T_TEXT}; font-size: 13px; font-weight: 700;")
        top.addWidget(self._title_lbl, 1)
        self._status_lbl = QLabel("")
        self._status_lbl.setObjectName("statusLabel")
        self._status_lbl.setStyleSheet(f"color: {T_SEC}; font-size: 11px;")
        top.addWidget(self._status_lbl)
        self._error_lbl = QLabel("")
        self._error_lbl.setObjectName("statusLabel")
        self._error_lbl.setStyleSheet(f"color: {T_ROSE2}; font-size: 11px;")
        self._error_lbl.hide()
        top.addWidget(self._error_lbl)
        top.addSpacing(8)
        self._browser_btn = QPushButton("Open in browser")
        self._browser_btn.setFixedHeight(28)
        self._browser_btn.setStyleSheet(
            f"background: transparent; color: {T_PRIMARY}; "
            f"border: 1px solid {T_PRIMARY}; border-radius: 6px; "
            "padding: 0 12px; font-size: 11px;")
        self._browser_btn.clicked.connect(self._open_browser)
        top.addWidget(self._browser_btn)
        self._close_btn = QPushButton("✕")
        self._close_btn.setObjectName("iconBtn")
        self._close_btn.setFixedSize(32, 32)
        self._close_btn.setStyleSheet(
            f"background: transparent; color: {T_SEC}; border: none; "
            "border-radius: 16px; font-size: 16px; padding: 0;")
        self._close_btn.clicked.connect(self.close_panel)
        top.addWidget(self._close_btn)
        cl.addLayout(top)

        # playback row
        bottom = QHBoxLayout()
        bottom.setSpacing(10)
        self._play_btn = QPushButton("▶")
        self._play_btn.setObjectName("iconBtn")
        self._play_btn.setFixedSize(36, 36)
        self._play_btn.setStyleSheet(
            f"background: {T_PRIMARY}; color: #000; border: none; "
            "border-radius: 18px; font-size: 14px; padding: 0;")
        self._play_btn.clicked.connect(self._toggle_play)
        bottom.addWidget(self._play_btn)

        self._position_slider = QSlider(Qt.Orientation.Horizontal)
        self._position_slider.setRange(0, 0)
        self._position_slider.sliderMoved.connect(self._seek)
        bottom.addWidget(self._position_slider, 1)

        self._time_lbl = QLabel("0:00")
        self._time_lbl.setStyleSheet(f"color: {T_SEC}; font-size: 11px;")
        self._time_lbl.setFixedWidth(50)
        bottom.addWidget(self._time_lbl)

        self._volume_btn = QPushButton("🔊")
        self._volume_btn.setObjectName("iconBtn")
        self._volume_btn.setFixedSize(30, 30)
        self._volume_btn.setStyleSheet(
            f"background: transparent; color: {T_SEC}; border: none; "
            "font-size: 14px;")
        self._volume_btn.clicked.connect(self._toggle_mute)
        bottom.addWidget(self._volume_btn)
        cl.addLayout(bottom)
        self._controls.hide()

        lay.addWidget(self._video_container)
        lay.addWidget(self._controls)

    # ---- streaming helpers ----------------------------------------------------

    def _slide_in(self) -> None:
        if self.height() == 532:
            return
        self._controls.show()
        # animate height 0 → 532
        def _grow():
            for h in range(0, 533, 33):
                self.setFixedHeight(h)
                QApplication.processEvents()
        t = threading.Thread(target=_grow, daemon=True)
        t.start()

    def _slide_out(self) -> None:
        def _shrink():
            for h in range(self.height(), -1, -33):
                self.setFixedHeight(max(0, h))
                QApplication.processEvents()
        t = threading.Thread(target=_shrink, daemon=True)
        t.start()

    def _start_extractor(self, url: str) -> None:
        self._cancel_extractor()
        self._extractor = _StreamExtractor(url, self)
        self._extractor.stream_ready.connect(self._on_stream_ready)
        self._extractor.error.connect(self._on_extractor_error)
        self._extractor.start()

    def _cancel_extractor(self) -> None:
        ex = self._extractor
        if ex is None:
            return
        ex.stop()
        try:
            ex.stream_ready.disconnect(self._on_stream_ready)
        except Exception:
            pass
        try:
            ex.error.disconnect(self._on_extractor_error)
        except Exception:
            pass
        self._extractor = None

    @pyqtSlot(str)
    def _on_stream_ready(self, url: str) -> None:
        self._stream_url = url
        self._status_lbl.setText("")
        self._controls.show()
        self._init_player(QUrl(url))

    @pyqtSlot(str)
    def _on_extractor_error(self, msg: str) -> None:
        # Fallback: try to open the source page in the browser
        self._error_lbl.setText(f"Stream unavailable: {msg}")
        self._error_lbl.show()
        self._browser_btn.setText("Open in browser")
        self._browser_btn.show()
        self._controls.show()
        if self._video_url:
            self._open_browser()

    def _open_browser(self) -> None:
        if self._video_url:
            QDesktopServices.openUrl(QUrl(self._video_url))
        self.close_panel()

    # ---- QtMultimedia player --------------------------------------------------

    def _init_player(self, url: QUrl) -> None:
        self._stop_player()

        try:
            from PyQt6.QtMultimedia import QAudioOutput, QMediaPlayer
            from PyQt6.QtMultimediaWidgets import QVideoWidget
        except ImportError:
            self._error_lbl.setText(
                "Playback unavailable: PyQt6 Multimedia not installed. "
                "Opening in browser instead.")
            self._error_lbl.show()
            if self._video_url:
                QTimer.singleShot(500,
                                  lambda: QDesktopServices.openUrl(
                                      QUrl(self._video_url)))
                self.close_panel()
            return

        self._player = QMediaPlayer()
        self._audio = QAudioOutput()
        self._audio.setVolume(1.0)
        self._player.setAudioOutput(self._audio)

        # Replace placeholder with real video widget
        if self._vid_widget is None:
            self._vid_widget = QVideoWidget()
            self._vid_widget.setMinimumHeight(480)
            self._vid_widget.setStyleSheet(f"background: {T_CANVAS};")
            self._vid_widget.hide()
            # Swap placeholder for video widget
            self._video_container.layout().removeWidget(self._placeholder)
            self._placeholder.hide()
            self._video_container.layout().addWidget(self._vid_widget)
            self._vid_widget.show()

        self._player.setVideoOutput(self._vid_widget)
        self._player.playbackStateChanged.connect(self._on_state_changed)
        self._player.positionChanged.connect(self._on_position_changed)
        self._player.durationChanged.connect(self._on_duration_changed)
        self._player.errorOccurred.connect(self._on_player_error)
        self._player.setSource(url)
        self._player.play()

    def _stop_player(self) -> None:
        p = self._player
        if p is not None:
            try:
                p.playbackStateChanged.disconnect(self._on_state_changed)
            except Exception:
                pass
            try:
                p.positionChanged.disconnect(self._on_position_changed)
            except Exception:
                pass
            try:
                p.durationChanged.disconnect(self._on_duration_changed)
            except Exception:
                pass
            try:
                p.errorOccurred.disconnect(self._on_player_error)
            except Exception:
                pass
            p.stop()
            p.deleteLater()
        self._player = None
        self._audio = None

    @pyqtSlot()
    def _on_state_changed(self) -> None:
        if self._player is None:
            return
        state = self._player.playbackState()
        if state == QMediaPlayer.PlaybackState.PlayingState:
            self._play_btn.setText("⏸")
        else:
            self._play_btn.setText("▶")

    @pyqtSlot(int)
    def _on_position_changed(self, pos: int) -> None:
        self._position_slider.setValue(pos)
        total = self._player.duration() if self._player else 0
        self._time_lbl.setText(f"{_fmt_time(pos)} / {_fmt_time(total)}")

    @pyqtSlot(int)
    def _on_duration_changed(self, dur: int) -> None:
        self._position_slider.setRange(0, max(0, dur))

    @pyqtSlot(int)
    def _on_player_error(self, err: int) -> None:
        # Error code 4 = No proper format — try browser fallback
        self._error_lbl.setText(
            "Playback error. Opening in your default browser instead.")
        self._error_lbl.show()
        if self._video_url:
            QTimer.singleShot(1000,
                              lambda: QDesktopServices.openUrl(
                                  QUrl(self._video_url)))
        self._player.stop()

    # ---- controls ------------------------------------------------------------

    def _toggle_play(self) -> None:
        if self._player is None:
            return
        if self._player.playbackState() == QMediaPlayer.PlaybackState.PlayingState:
            self._player.pause()
        else:
            self._player.play()

    def _seek(self, pos: int) -> None:
        if self._player is not None:
            self._player.setPosition(pos)

    def _toggle_mute(self) -> None:
        if self._audio is None:
            return
        vol = self._audio.volume()
        if vol > 0:
            self._audio.setVolume(0)
            self._volume_btn.setText("🔇")
        else:
            self._audio.setVolume(1.0)
            self._volume_btn.setText("🔊")

    # ---- shutdown ------------------------------------------------------------

    def shutdown(self) -> None:
        self._cancel_extractor()
        self._stop_player()


def _fmt_time(ms: int) -> str:
    s = max(0, ms // 1000)
    m, sec = divmod(s, 60)
    h, m = divmod(m, 60)
    if h:
        return f"{h}:{m:02}:{sec:02}"
    return f"{m}:{sec:02}"
