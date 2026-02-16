from __future__ import annotations

import os
import queue
import re
import threading
import tkinter as tk
from datetime import datetime
from pathlib import Path
from tkinter import messagebox, scrolledtext, ttk
from typing import Callable

from .config import (
    ActiveTimeConfig,
    AppConfig,
    BandConfig,
    ConfigError,
    LoggingConfig,
    NaverConfig,
    RetryConfig,
    ScheduleConfig,
    StyleConfig,
    load_config,
    save_config,
)
from .main import run_dry, run_once, run_service


SERVICE_STOPPED = "__SERVICE_STOPPED__"


class BandAutoPosterGUI:
    def __init__(self, root: tk.Tk, config_path: str = "config.yaml") -> None:
        self.root = root
        self.config_path = config_path
        self.stop_event = threading.Event()
        self.worker_thread: threading.Thread | None = None
        self.log_queue: queue.Queue[str] = queue.Queue()

        self.root.title("NaverBand 자동 게시 도우미")
        self.root.geometry("880x680")

        self._build_layout()
        self._load_into_form()
        self._poll_logs()
        self.root.protocol("WM_DELETE_WINDOW", self.on_close)

    def _build_layout(self) -> None:
        frame = ttk.Frame(self.root, padding=12)
        frame.pack(fill="both", expand=True)

        ttk.Label(frame, text="네이버 밴드 자동 게시 (GUI 전용)", font=("Arial", 16, "bold")).pack(anchor="w")
        ttk.Label(
            frame,
            text="설정/시작/중지 모두 이 화면에서 진행할 수 있습니다.",
            foreground="#444",
        ).pack(anchor="w", pady=(0, 8))

        form = ttk.LabelFrame(frame, text="작업 설정", padding=10)
        form.pack(fill="x", pady=(0, 10))

        self.band_url_var = tk.StringVar()
        self.interval_var = tk.StringVar()
        self.start_var = tk.StringVar()
        self.end_var = tk.StringVar()
        self.timezone_var = tk.StringVar()
        self.bold_var = tk.BooleanVar(value=True)
        self.font_size_var = tk.StringVar()
        self.font_color_var = tk.StringVar()
        self.max_attempts_var = tk.StringVar()
        self.backoff_var = tk.StringVar()
        self.naver_id_var = tk.StringVar()
        self.naver_pw_var = tk.StringVar()

        self._add_entry(form, 0, "밴드 URL", self.band_url_var)
        self._add_entry(form, 1, "게시 주기(분)", self.interval_var)
        self._add_entry(form, 2, "시작 시간(HH:MM)", self.start_var)
        self._add_entry(form, 3, "종료 시간(HH:MM)", self.end_var)
        self._add_entry(form, 4, "타임존", self.timezone_var)
        self._add_entry(form, 5, "폰트 크기", self.font_size_var)
        self._add_entry(form, 6, "폰트 색상(#RRGGBB)", self.font_color_var)
        self._add_entry(form, 7, "최대 재시도", self.max_attempts_var)
        self._add_entry(form, 8, "재시도 간격(초)", self.backoff_var)
        self._add_entry(form, 9, "네이버 ID", self.naver_id_var)
        self._add_entry(form, 10, "네이버 PW", self.naver_pw_var, show="*")

        ttk.Checkbutton(form, text="굵게(Bold)", variable=self.bold_var).grid(row=11, column=0, sticky="w", pady=4)

        ttk.Label(form, text="게시 내용").grid(row=12, column=0, sticky="nw", pady=4)
        self.post_text_widget = scrolledtext.ScrolledText(form, height=6)
        self.post_text_widget.grid(row=12, column=1, sticky="ew", pady=4)
        form.columnconfigure(1, weight=1)

        btn_row = ttk.Frame(frame)
        btn_row.pack(fill="x", pady=(0, 10))

        ttk.Button(btn_row, text="설정 저장", command=self.handle_save).pack(side="left", padx=(0, 8))
        ttk.Button(btn_row, text="설정 점검", command=self.handle_dry_run).pack(side="left", padx=(0, 8))
        ttk.Button(btn_row, text="한 번 실행", command=self.handle_run_once).pack(side="left", padx=(0, 8))
        self.start_btn = ttk.Button(btn_row, text="자동 시작", command=self.handle_start_service)
        self.start_btn.pack(side="left", padx=(0, 8))
        self.stop_btn = ttk.Button(btn_row, text="자동 중지", command=self.handle_stop_service, state="disabled")
        self.stop_btn.pack(side="left")

        self.status_var = tk.StringVar(value="상태: 대기 중")
        ttk.Label(frame, textvariable=self.status_var, font=("Arial", 11, "bold")).pack(anchor="w", pady=(0, 8))

        self.log_view = scrolledtext.ScrolledText(frame, wrap="word", height=12, state="disabled")
        self.log_view.pack(fill="both", expand=True)

    def _add_entry(self, parent: ttk.LabelFrame, row: int, label: str, var: tk.StringVar, show: str | None = None) -> None:
        ttk.Label(parent, text=label).grid(row=row, column=0, sticky="w", pady=3)
        ttk.Entry(parent, textvariable=var, show=show if show else "").grid(row=row, column=1, sticky="ew", pady=3)

    def _load_into_form(self) -> None:
        path = Path(self.config_path)
        if not path.exists():
            example = Path("config.example.yaml")
            if example.exists():
                path.write_text(example.read_text(encoding="utf-8"), encoding="utf-8")

        if not path.exists():
            self._enqueue_log("config.yaml과 config.example.yaml이 없어 기본값으로 시작합니다.")
            self._set_defaults()
            return

        cfg = load_config(path)
        self.band_url_var.set(cfg.band.target_url)
        self.interval_var.set(str(cfg.schedule.interval_minutes))
        self.start_var.set(cfg.schedule.active_time.start)
        self.end_var.set(cfg.schedule.active_time.end)
        self.timezone_var.set(cfg.schedule.timezone)
        self.bold_var.set(cfg.band.style.bold)
        self.font_size_var.set(str(cfg.band.style.font_size))
        self.font_color_var.set(cfg.band.style.font_color)
        self.max_attempts_var.set(str(cfg.retry.max_attempts))
        self.backoff_var.set(str(cfg.retry.backoff_seconds))
        self.post_text_widget.delete("1.0", "end")
        self.post_text_widget.insert("1.0", cfg.band.post_text)

        self.naver_id_var.set(os.getenv(cfg.naver.username_env, ""))
        self.naver_pw_var.set(os.getenv(cfg.naver.password_env, ""))
        self._enqueue_log(f"설정 파일 로드 완료: {self.config_path}")

    def _set_defaults(self) -> None:
        self.band_url_var.set("https://band.us/band/your-band-id")
        self.interval_var.set("5")
        self.start_var.set("07:00")
        self.end_var.set("19:00")
        self.timezone_var.set("Asia/Seoul")
        self.bold_var.set(True)
        self.font_size_var.set("16")
        self.font_color_var.set("#1F6FEB")
        self.max_attempts_var.set("3")
        self.backoff_var.set("8")
        self.post_text_widget.delete("1.0", "end")
        self.post_text_widget.insert("1.0", "정기 공지입니다.")

    def _validate_inputs(self) -> None:
        if not self.band_url_var.get().strip():
            raise ConfigError("밴드 URL을 입력하세요.")
        if not self.post_text_widget.get("1.0", "end").strip():
            raise ConfigError("게시 내용을 입력하세요.")

        for field_name, value in {
            "시작 시간": self.start_var.get().strip(),
            "종료 시간": self.end_var.get().strip(),
        }.items():
            if not re.fullmatch(r"\d{2}:\d{2}", value):
                raise ConfigError(f"{field_name} 형식은 HH:MM 이어야 합니다.")

        color = self.font_color_var.get().strip()
        if not re.fullmatch(r"#[0-9a-fA-F]{6}", color):
            raise ConfigError("폰트 색상은 #RRGGBB 형식이어야 합니다.")

        if int(self.interval_var.get().strip()) < 1:
            raise ConfigError("게시 주기는 1분 이상이어야 합니다.")

    def _build_config_from_form(self) -> AppConfig:
        self._validate_inputs()
        post_text = self.post_text_widget.get("1.0", "end").strip()

        return AppConfig(
            naver=NaverConfig(
                username_env="NAVER_ID",
                password_env="NAVER_PW",
                use_saved_session=True,
                session_file="./data/session.json",
            ),
            band=BandConfig(
                target_url=self.band_url_var.get().strip(),
                post_text=post_text,
                style=StyleConfig(
                    bold=self.bold_var.get(),
                    font_size=int(self.font_size_var.get().strip()),
                    font_color=self.font_color_var.get().strip(),
                ),
            ),
            schedule=ScheduleConfig(
                interval_minutes=int(self.interval_var.get().strip()),
                timezone=self.timezone_var.get().strip(),
                active_time=ActiveTimeConfig(
                    start=self.start_var.get().strip(),
                    end=self.end_var.get().strip(),
                ),
            ),
            retry=RetryConfig(
                max_attempts=int(self.max_attempts_var.get().strip()),
                backoff_seconds=int(self.backoff_var.get().strip()),
            ),
            logging=LoggingConfig(level="INFO", file="./logs/app.log"),
        )

    def _apply_credentials_env(self, cfg: AppConfig) -> None:
        os.environ[cfg.naver.username_env] = self.naver_id_var.get().strip()
        os.environ[cfg.naver.password_env] = self.naver_pw_var.get().strip()

    def handle_save(self) -> None:
        try:
            cfg = self._build_config_from_form()
            save_config(self.config_path, cfg)
            self._enqueue_log(f"설정 저장 완료: {self.config_path}")
        except Exception as exc:
            messagebox.showerror("설정 오류", str(exc))

    def _run_thread(self, target: Callable[[], None], running_text: str) -> None:
        if self.worker_thread and self.worker_thread.is_alive():
            messagebox.showinfo("안내", "이미 실행 중입니다.")
            return
        self._set_status(running_text)
        self.worker_thread = threading.Thread(target=target, daemon=True)
        self.worker_thread.start()

    def handle_dry_run(self) -> None:
        def _work() -> None:
            try:
                cfg = self._build_config_from_form()
                self._apply_credentials_env(cfg)
                run_dry(cfg, status_callback=self._enqueue_log)
            except Exception as exc:
                self._enqueue_log(f"오류: {exc}")

        self._run_thread(_work, "설정 점검 실행 중...")

    def handle_run_once(self) -> None:
        def _work() -> None:
            try:
                cfg = self._build_config_from_form()
                self._apply_credentials_env(cfg)
                run_once(cfg, status_callback=self._enqueue_log)
            except Exception as exc:
                self._enqueue_log(f"오류: {exc}")

        self._run_thread(_work, "1회 실행 중...")

    def handle_start_service(self) -> None:
        def _work() -> None:
            try:
                cfg = self._build_config_from_form()
                self._apply_credentials_env(cfg)
                self.stop_event.clear()
                run_service(cfg, stop_event=self.stop_event, status_callback=self._enqueue_log)
            except Exception as exc:
                self._enqueue_log(f"오류: {exc}")
            finally:
                self.log_queue.put(SERVICE_STOPPED)

        if self.worker_thread and self.worker_thread.is_alive():
            messagebox.showinfo("안내", "이미 자동 실행 중입니다.")
            return

        self.start_btn.configure(state="disabled")
        self.stop_btn.configure(state="normal")
        self._run_thread(_work, "자동 실행 중")

    def handle_stop_service(self) -> None:
        self.stop_event.set()
        self._set_status("중지 요청됨")

    def _enqueue_log(self, message: str) -> None:
        ts = datetime.now().strftime("%H:%M:%S")
        self.log_queue.put(f"[{ts}] {message}")

    def _poll_logs(self) -> None:
        while not self.log_queue.empty():
            item = self.log_queue.get()
            if item == SERVICE_STOPPED:
                self.start_btn.configure(state="normal")
                self.stop_btn.configure(state="disabled")
                continue

            self._append_log(item)
            self._set_status(item)
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


def launch_gui(config_path: str = "config.yaml") -> None:
    root = tk.Tk()
    BandAutoPosterGUI(root, config_path=config_path)
    root.mainloop()
