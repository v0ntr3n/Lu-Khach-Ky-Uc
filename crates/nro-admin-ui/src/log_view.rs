//! Live Server Logs tab with severity filters, keyword search, pause/resume, and clear.

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::label::Label;
use gpui_kit::*;

use crate::app::{AdminApp, LogLevelFilter};
use crate::log_buffer::LogLevel;

const MAX_RENDERED_LOG_ROWS: usize = 250;

fn level_matches(filter: LogLevelFilter, level: LogLevel) -> bool {
    match filter {
        LogLevelFilter::All => true,
        LogLevelFilter::Info => matches!(level, LogLevel::Info | LogLevel::Warn | LogLevel::Error),
        LogLevelFilter::Warn => matches!(level, LogLevel::Warn | LogLevel::Error),
        LogLevelFilter::Error => matches!(level, LogLevel::Error),
    }
}

fn level_button(id: &'static str, label: &'static str, active: bool) -> Button {
    let btn = Button::new(id).label(label);
    if active { btn.primary() } else { btn }
}

impl AdminApp {
    pub(crate) fn render_log_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let keyword = self.log_search_input.read(cx).value().trim().to_lowercase();

        let filtered: Vec<_> = self
            .logs
            .iter()
            .filter(|entry| {
                level_matches(self.log_level_filter, entry.level)
                    && (keyword.is_empty() || entry.text.to_lowercase().contains(&keyword))
            })
            .collect();

        let mut list = div()
            .id("log-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_3()
            .gap_1()
            .rounded_md()
            .border_1()
            .border_color(border);

        if filtered.is_empty() {
            list = list.child(Label::new("Chưa có dòng log nào phù hợp bộ lọc."));
        } else {
            let start = filtered.len().saturating_sub(MAX_RENDERED_LOG_ROWS);
            for entry in &filtered[start..] {
                let color = match entry.level {
                    LogLevel::Error => rgb(0xef4444).into(),
                    LogLevel::Warn => rgb(0xf59e0b).into(),
                    LogLevel::Info => fg,
                    LogLevel::Debug => muted,
                };
                list = list.child(
                    div()
                        .id(("log-line", usize::try_from(entry.seq).unwrap_or(0)))
                        .text_color(color)
                        .child(entry.text.clone()),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        level_button(
                            "log-lvl-all",
                            "Tất cả",
                            self.log_level_filter == LogLevelFilter::All,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.log_level_filter = LogLevelFilter::All;
                            cx.notify();
                        })),
                    )
                    .child(
                        level_button(
                            "log-lvl-info",
                            "INFO+",
                            self.log_level_filter == LogLevelFilter::Info,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.log_level_filter = LogLevelFilter::Info;
                            cx.notify();
                        })),
                    )
                    .child(
                        level_button(
                            "log-lvl-warn",
                            "WARN+",
                            self.log_level_filter == LogLevelFilter::Warn,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.log_level_filter = LogLevelFilter::Warn;
                            cx.notify();
                        })),
                    )
                    .child(
                        level_button(
                            "log-lvl-err",
                            "ERROR",
                            self.log_level_filter == LogLevelFilter::Error,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.log_level_filter = LogLevelFilter::Error;
                            cx.notify();
                        })),
                    )
                    .child(div().flex_1().child(Input::new(&self.log_search_input)))
                    .child(
                        Button::new("btn-log-pause")
                            .label(if self.log_paused {
                                "Tiếp tục"
                            } else {
                                "Tạm dừng"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.log_paused = !this.log_paused;
                                if !this.log_paused
                                    && let Some((seq, entries, warn_err)) =
                                        this.log_buffer.snapshot_if_changed(this.last_log_seq)
                                {
                                    this.last_log_seq = seq;
                                    this.logs = entries;
                                    this.log_warn_err_count = warn_err;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("btn-log-clear")
                            .danger()
                            .label("Xóa log")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.log_buffer.clear();
                                this.logs.clear();
                                this.log_warn_err_count = 0;
                                cx.notify();
                            })),
                    )
                    .child(Label::new(format!(
                        "{}/{} dòng",
                        filtered.len().min(MAX_RENDERED_LOG_ROWS),
                        self.logs.len()
                    ))),
            )
            .child(list)
    }
}
