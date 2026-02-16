from __future__ import annotations

import queue
import threading
import tkinter as tk
from datetime import datetime
from tkinter import messagebox, scrolledtext, ttk

from .config import AppConfig
from .main import run_dry, run_once, run_service


class BandAutoPosterGUI:
    def __init__(self, root: tk.Tk, cfg: AppConfig) -> None:
        self.root = root
        self.cfg = cfg
        self.stop_event = threading.Event()
        self.worker_thread: threading.Thread | None = None
        self.log_queue: queue.Queue[str] = queue.Queue()

        self.root.title("NaverBand 자동 게시 도우미")
        self.root.geometry("720x520")

        self._build_layout()
        self._poll_logs()
        self.root.protocol("WM_DELETE_WINDOW", self.on_close)

    def _build_layout(self) -> None:
        frame = ttk.Frame(self.root, padding=12)
        frame.pack(fill="both", expand=True)

        title = ttk.Label(frame, text="네이버 밴드 자동 게시", font=("Arial", 16, "bold"))
        title.pack(anchor="w", pady=(0, 8))

        desc = ttk.Label(
            frame,
            text=(
                "초심자용 실행 화면입니다.\n"
                "- [한 번 실행] : 지금 즉시 1회 게시\n"
                "- [자동 시작] : 설정한 주기로 백그라운드 반복 게시\n"
                "- [자동 중지] : 반복 게시 중지"
            ),
            justify="left",
        )
        desc.pack(anchor="w", pady=(0, 10))

        cfg_text = (
            f"대상 밴드: {self.cfg.band.target_url}\n"
            f"주기: {self.cfg.schedule.interval_minutes}분\n"
            f"활성 시간: {self.cfg.schedule.active_time.start} ~ {self.cfg.schedule.active_time.end}"
        )
        ttk.Label(frame, text=cfg_text, foreground="#333333").pack(anchor="w", pady=(0, 8))

        button_row = ttk.Frame(frame)
        button_row.pack(fill="x", pady=(0, 10))

        self.dry_run_btn = ttk.Button(button_row, text="설정 점검", command=self.handle_dry_run)
        self.dry_run_btn.pack(side="left", padx=(0, 8))

        self.run_once_btn = ttk.Button(button_row, text="한 번 실행", command=self.handle_run_once)
        self.run_once_btn.pack(side="left", padx=(0, 8))

        self.start_btn = ttk.Button(button_row, text="자동 시작", command=self.handle_start_service)
        self.start_btn.pack(side="left", padx=(0, 8))

        self.stop_btn = ttk.Button(button_row, text="자동 중지", command=self.handle_stop_service, state="disabled")
        self.stop_btn.pack(side="left")

        self.status_var = tk.StringVar(value="대기 중")
        ttk.Label(frame, textvariable=self.status_var, font=("Arial", 11, "bold")).pack(anchor="w", pady=(0, 8))

        self.log_view = scrolledtext.ScrolledText(frame, wrap="word", height=16, state="disabled")
        self.log_view.pack(fill="both", expand=True)

    def handle_dry_run(self) -> None:
        if self.worker_thread and self.worker_thread.is_alive():
            messagebox.showinfo("안내", "이미 실행 중입니다.")
            return

        self._set_status("설정 점검 실행 중...")
        self.worker_thread = threading.Thread(target=self._dry_run_worker, daemon=True)
        self.worker_thread.start()

    def _dry_run_worker(self) -> None:
        try:
            run_dry(self.cfg, status_callback=self._enqueue_log)
        except Exception as exc:
            self._enqueue_log(f"오류: {exc}")

    def handle_run_once(self) -> None:
        if self.worker_thread and self.worker_thread.is_alive():
            messagebox.showinfo("안내", "이미 실행 중입니다.")
            return

        self._set_status("1회 실행 중...")
        self.worker_thread = threading.Thread(target=self._run_once_worker, daemon=True)
        self.worker_thread.start()

    def _run_once_worker(self) -> None:
        try:
            run_once(self.cfg, status_callback=self._enqueue_log)
        except Exception as exc:
            self._enqueue_log(f"오류: {exc}")

    def handle_start_service(self) -> None:
        if self.worker_thread and self.worker_thread.is_alive():
            messagebox.showinfo("안내", "이미 자동 실행 중입니다.")
            return

        self.stop_event.clear()
        self.worker_thread = threading.Thread(target=self._service_worker, daemon=True)
        self.worker_thread.start()

        self.start_btn.configure(state="disabled")
        self.stop_btn.configure(state="normal")
        self._set_status("자동 실행 중")

    def _service_worker(self) -> None:
        run_service(self.cfg, stop_event=self.stop_event, status_callback=self._enqueue_log)

    def handle_stop_service(self) -> None:
        if self.worker_thread and self.worker_thread.is_alive():
            self.stop_event.set()
            self._set_status("중지 요청됨")
        self.start_btn.configure(state="normal")
        self.stop_btn.configure(state="disabled")

    def _enqueue_log(self, message: str) -> None:
        ts = datetime.now().strftime("%H:%M:%S")
        self.log_queue.put(f"[{ts}] {message}")

    def _poll_logs(self) -> None:
        while not self.log_queue.empty():
            log = self.log_queue.get()
            self._append_log(log)
            self._set_status(log)
        self.root.after(300, self._poll_logs)

    def _append_log(self, text: str) -> None:
        self.log_view.configure(state="normal")
        self.log_view.insert("end", f"{text}\n")
        self.log_view.see("end")
        self.log_view.configure(state="disabled")

    def _set_status(self, text: str) -> None:
        self.status_var.set(f"상태: {text}")

    def on_close(self) -> None:
        self.stop_event.set()
        self.root.destroy()


def launch_gui(cfg: AppConfig) -> None:
    root = tk.Tk()
    BandAutoPosterGUI(root, cfg)
    root.mainloop()
