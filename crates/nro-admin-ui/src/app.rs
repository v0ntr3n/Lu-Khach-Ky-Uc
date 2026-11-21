//! Main GPUI Kit dashboard & user control state for `nro-server-gui`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Local;
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::*;
use nro_server::admin_control::{AdminControl, AdminUserRow, ServerSummary};

const MAX_AUDIT_LOGS: usize = 40;
const SEARCH_DEBOUNCE_MS: u64 = 250;

pub(crate) enum BootState {
    Booting,
    Ready(AdminControl),
    Failed(String),
}

pub struct AdminApp {
    boot_state: Arc<Mutex<BootState>>,
    pub(crate) control: Option<AdminControl>,
    pub(crate) boot_error: Option<String>,
    pub(crate) summary: Option<ServerSummary>,
    pub(crate) online_count: usize,
    pub(crate) bind_addr: String,
    pub(crate) users: Vec<AdminUserRow>,
    pub(crate) selected_account: Option<i32>,
    pub(crate) show_offline: bool,
    is_refreshing: bool,
    refresh_pending: bool,
    pub(crate) action_in_flight: bool,
    shutting_down: bool,
    search_debounce_gen: u64,
    pub(crate) status_banner: SharedString,
    pub(crate) audit_logs: VecDeque<String>,
    pub(crate) search_input: Entity<InputState>,
    pub(crate) broadcast_input: Entity<InputState>,
    pub(crate) gold_input: Entity<InputState>,
    pub(crate) gem_input: Entity<InputState>,
    pub(crate) vnd_input: Entity<InputState>,
    pub(crate) item_id_input: Entity<InputState>,
    pub(crate) item_qty_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl AdminApp {
    pub(crate) fn new(
        boot_state: Arc<Mutex<BootState>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Tìm theo ID / username / tên nhân vật...")
        });
        let broadcast_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Nhập thông báo gửi toàn server..."));
        let gold_input = cx.new(|cx| InputState::new(window, cx).placeholder("Vàng (+/-)"));
        let gem_input = cx.new(|cx| InputState::new(window, cx).placeholder("Ngọc (+/-)"));
        let vnd_input = cx.new(|cx| InputState::new(window, cx).placeholder("VND (+/-)"));
        let item_id_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Item Template ID"));
        let item_qty_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("SL (mặc định 1)"));

        let sub = cx.subscribe_in(&search_input, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.schedule_debounced_refresh(cx);
            }
        });

        let mut audit_logs = VecDeque::with_capacity(MAX_AUDIT_LOGS);
        audit_logs.push_front(format!(
            "[{}] Đang khởi động server Lữ Khách Ký Ức...",
            Local::now().format("%H:%M:%S")
        ));

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

        let mut app = Self {
            boot_state,
            control: None,
            boot_error: None,
            summary: None,
            online_count: 0,
            bind_addr: String::new(),
            users: Vec::new(),
            selected_account: None,
            show_offline: false,
            is_refreshing: false,
            refresh_pending: false,
            action_in_flight: false,
            shutting_down: false,
            search_debounce_gen: 0,
            status_banner: "Đang khởi động server...".into(),
            audit_logs,
            search_input,
            broadcast_input,
            gold_input,
            gem_input,
            vnd_input,
            item_id_input,
            item_qty_input,
            _subscriptions: vec![sub],
        };
        app.poll_boot_and_refresh(cx);
        app
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

    fn schedule_debounced_refresh(&mut self, cx: &mut Context<Self>) {
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
        let online_only = !self.show_offline;
        cx.spawn(async move |this, cx| {
            let (summary, users) = cx
                .background_executor()
                .spawn(async move { (control.summary(), control.list_users(&query, online_only)) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.is_refreshing = false;
                if !app.shutting_down {
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
            let res = cx
                .background_executor()
                .spawn(async move { work(control) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.action_in_flight = false;
                match res {
                    Ok(msg) | Err(msg) => app.push_feedback(msg),
                }
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
        self.push_feedback("Đang lưu dữ liệu và tắt server...".to_owned());
        cx.quit();
    }

    pub(crate) fn push_feedback(&mut self, msg: String) {
        let line = format!("[{}] {msg}", Local::now().format("%H:%M:%S"));
        if self.audit_logs.len() >= MAX_AUDIT_LOGS {
            self.audit_logs.pop_back();
        }
        self.audit_logs.push_front(line);
        self.status_banner = msg.into();
    }

    pub(crate) fn selected_user(&self) -> Option<&AdminUserRow> {
        let aid = self.selected_account?;
        self.users.iter().find(|u| u.account_id == aid)
    }
}

impl Render for AdminApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (bg, fg) = (cx.theme().background, cx.theme().foreground);
        let body = if let Some(err) = &self.boot_error {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .p_6()
                .gap_3()
                .child(Label::new("Không thể khởi động server nhúng:"))
                .child(Label::new(err.clone()))
                .child(Label::new(
                    "Gợi ý: Đặt nro-server-gui.exe vào thư mục gốc Lu-Khach-Ky-Uc (cùng chỗ với server.toml và data/) và đảm bảo không có nro-server.exe khác đang chiếm cổng 14445.",
                ))
        } else {
            div()
                .flex()
                .flex_row()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .child(self.render_user_list(cx))
                .child(self.render_inspector(cx))
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .bg(bg)
            .text_color(fg)
            .child(self.render_header(cx))
            .child(self.render_broadcast_bar(cx))
            .child(body)
            .child(self.render_footer(cx))
    }
}
