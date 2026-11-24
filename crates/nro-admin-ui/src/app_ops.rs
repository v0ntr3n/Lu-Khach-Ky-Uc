//! Background polling, debounced refreshes, item search, and async admin actions for `AdminApp`.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::Ordering;
use std::time::Duration;

use chrono::Local;
use gpui_kit::*;
use nro_server::admin_control::AdminControl;

use crate::app::{AdminApp, BootState, MAX_AUDIT_LOGS, UserFilter};

const SEARCH_DEBOUNCE_MS: u64 = 250;
const LOG_POLL_MS: u64 = 500;
const ITEM_SEARCH_LIMIT: usize = 12;

impl AdminApp {
    pub(crate) fn spawn_user_poll(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let state = this.read_with(cx, |app, _| {
                    (
                        app.shutting_down,
                        app.control.is_none() && app.boot_error.is_none(),
                    )
                });
                let Ok((shutting_down, is_booting)) = state else {
                    break;
                };
                if shutting_down {
                    break;
                }
                let delay = if is_booting {
                    Duration::from_millis(500)
                } else {
                    Duration::from_secs(3)
                };
                cx.background_executor().timer(delay).await;
                let keep = this.update(cx, |app, cx| {
                    if app.shutting_down {
                        return false;
                    }
                    app.poll_boot_and_refresh(cx);
                    true
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn spawn_log_poll(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let state = this.read_with(cx, |app, _| {
                    (
                        app.shutting_down,
                        app.log_paused,
                        app.last_log_seq,
                        app.log_buffer.clone(),
                    )
                });
                let Ok((shutting_down, paused, last_seq, buf)) = state else {
                    break;
                };
                if shutting_down {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(LOG_POLL_MS))
                    .await;
                if paused {
                    continue;
                }
                if let Some((seq, entries, warn_err)) = buf.snapshot_if_changed(last_seq) {
                    let keep = this.update(cx, |app, cx| {
                        if app.shutting_down {
                            return false;
                        }
                        if !app.log_paused {
                            app.last_log_seq = seq;
                            app.logs = entries;
                            app.log_warn_err_count = warn_err;
                            cx.notify();
                        }
                        true
                    });
                    if !matches!(keep, Ok(true)) {
                        break;
                    }
                }
            }
        })
        .detach();
    }

    pub(crate) fn poll_boot_and_refresh(&mut self, cx: &mut Context<Self>) {
        if self.shutting_down {
            return;
        }
        if self.control.is_none() && self.boot_error.is_none() {
            let next = self.boot_state.lock().ok().and_then(|guard| match &*guard {
                BootState::Booting => None,
                BootState::Ready(ctrl) => Some(Ok(ctrl.clone())),
                BootState::Failed(err) => Some(Err(err.clone())),
            });
            match next {
                Some(Ok(ctrl)) => {
                    self.control = Some(ctrl);
                    self.push_feedback("Server đã sẵn sàng, đang tải dữ liệu...".to_owned());
                    self.refresh_item_results(cx);
                    cx.notify();
                }
                Some(Err(err)) => {
                    self.boot_error = Some(err.clone());
                    self.push_feedback(format!("Lỗi khởi động server: {err}"));
                    cx.notify();
                }
                None => {}
            }
        }
        self.spawn_refresh(cx);
    }

    pub(crate) fn schedule_debounced_refresh(&mut self, cx: &mut Context<Self>) {
        if self.shutting_down {
            return;
        }
        self.search_debounce_gen = self.search_debounce_gen.wrapping_add(1);
        let expected_gen = self.search_debounce_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(SEARCH_DEBOUNCE_MS))
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.search_debounce_gen == expected_gen && !app.shutting_down {
                    app.spawn_refresh(cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn refresh_item_results(&mut self, cx: &mut Context<Self>) {
        if self.shutting_down {
            return;
        }
        let Some(control) = self.control.clone() else {
            return;
        };
        self.item_search_gen = self.item_search_gen.wrapping_add(1);
        let expected_gen = self.item_search_gen;
        let query = self.item_search_input.read(cx).value().to_string();
        cx.spawn(async move |this, cx| {
            let results = cx
                .background_executor()
                .spawn(async move { control.search_items(&query, ITEM_SEARCH_LIMIT) })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.item_search_gen == expected_gen && !app.shutting_down {
                    app.item_results = results;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn spawn_refresh(&mut self, cx: &mut Context<Self>) {
        if self.shutting_down {
            return;
        }
        if self.is_refreshing {
            self.refresh_pending = true;
            return;
        }
        let Some(control) = self.control.clone() else {
            return;
        };
        self.is_refreshing = true;
        self.refresh_pending = false;
        let query = self.search_input.read(cx).value().to_string();
        let online_only = self.user_filter == UserFilter::Online;
        let banned_only = self.user_filter == UserFilter::Banned;
        cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move {
                    catch_unwind(AssertUnwindSafe(|| {
                        (
                            control.summary(),
                            control.list_users_filtered(&query, online_only, banned_only),
                        )
                    }))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.is_refreshing = false;
                if !app.shutting_down
                    && let Ok((summary, users)) = outcome
                {
                    let first_ready = app.summary.is_none();
                    app.online_count = summary.online_players;
                    app.bind_addr.clone_from(&summary.bind_addr);
                    if first_ready {
                        app.push_feedback(format!("Server đã sẵn sàng tại {}", summary.bind_addr));
                    }
                    app.summary = Some(summary);
                    app.users = users;
                    if let Some(sel) = app.selected_account
                        && !app.users.iter().any(|u| u.account_id == sel)
                    {
                        app.selected_account = app.users.first().map(|u| u.account_id);
                    } else if app.selected_account.is_none() {
                        app.selected_account = app.users.first().map(|u| u.account_id);
                    }
                }
                let should_rerun = app.refresh_pending && !app.shutting_down;
                app.refresh_pending = false;
                cx.notify();
                if should_rerun {
                    app.spawn_refresh(cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn run_async_action<F>(&mut self, cx: &mut Context<Self>, work: F)
    where
        F: FnOnce(AdminControl) -> Result<String, String> + Send + 'static,
    {
        if self.shutting_down {
            return;
        }
        if self.action_in_flight {
            self.push_feedback("Đang xử lý thao tác trước đó, vui lòng chờ...".to_owned());
            cx.notify();
            return;
        }
        let Some(control) = self.control.clone() else {
            self.push_feedback("Server chưa sẵn sàng.".to_owned());
            cx.notify();
            return;
        };
        self.action_in_flight = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move { catch_unwind(AssertUnwindSafe(|| work(control))) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.action_in_flight = false;
                let msg = match outcome {
                    Ok(Ok(ok_msg) | Err(ok_msg)) => ok_msg,
                    Err(_) => "Thao tác gặp lỗi nội bộ (panic đã được chặn an toàn).".to_owned(),
                };
                app.push_feedback(msg);
                if !app.shutting_down {
                    app.spawn_refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn request_shutdown(&mut self, cx: &mut Context<Self>) {
        if self.shutting_down {
            return;
        }
        self.shutting_down = true;
        self.refresh_pending = false;
        if let Some(ctrl) = &self.control {
            ctrl.trigger_shutdown();
        }
        self.window_closed.store(true, Ordering::Release);
        self.push_feedback("Đang lưu dữ liệu và tắt server...".to_owned());
        cx.quit();
    }

    pub(crate) fn push_feedback(&mut self, msg: String) {
        let ts = Local::now().format("%H:%M:%S").to_string();
        if self.audit_log.len() >= MAX_AUDIT_LOGS {
            self.audit_log.pop();
        }
        self.audit_log.insert(0, (ts, msg.clone()));
        self.status_banner = msg.into();
    }
}
