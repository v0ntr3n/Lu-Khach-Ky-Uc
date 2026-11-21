//! Main GPUI Kit dashboard & user control view for `nro-server-gui`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Local;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::{ActiveTheme, IconName};
use gpui_kit::*;
use nro_server::admin_control::{AdminControl, AdminUserRow, ServerSummary};

const MAX_AUDIT_LOGS: usize = 40;

pub(crate) enum BootState {
    Booting,
    Ready(AdminControl),
    Failed(String),
}

pub struct AdminApp {
    boot_state: Arc<Mutex<BootState>>,
    pub(crate) control: Option<AdminControl>,
    boot_error: Option<String>,
    summary: Option<ServerSummary>,
    users: Vec<AdminUserRow>,
    selected_account: Option<i32>,
    online_only: bool,
    status_banner: SharedString,
    audit_logs: VecDeque<String>,
    search_input: Entity<InputState>,
    broadcast_input: Entity<InputState>,
    pub(crate) gold_input: Entity<InputState>,
    pub(crate) gem_input: Entity<InputState>,
    pub(crate) vnd_input: Entity<InputState>,
    pub(crate) item_id_input: Entity<InputState>,
    pub(crate) item_qty_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl AdminApp {
    pub(crate) fn new(boot_state: Arc<Mutex<BootState>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Tìm theo ID / username / tên nhân vật..."));
        let broadcast_input = cx.new(|cx| InputState::new(window, cx).placeholder("Nhập thông báo gửi toàn server..."));
        let gold_input = cx.new(|cx| InputState::new(window, cx).placeholder("Vàng (+/-)"));
        let gem_input = cx.new(|cx| InputState::new(window, cx).placeholder("Ngọc (+/-)"));
        let vnd_input = cx.new(|cx| InputState::new(window, cx).placeholder("VND (+/-)"));
        let item_id_input = cx.new(|cx| InputState::new(window, cx).placeholder("Item Template ID"));
        let item_qty_input = cx.new(|cx| InputState::new(window, cx).placeholder("SL (mặc định 1)"));

        let sub = cx.subscribe_in(&search_input, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh_data(cx);
                cx.notify();
            }
        });

        let mut audit_logs = VecDeque::with_capacity(MAX_AUDIT_LOGS);
        audit_logs.push_front(format!(
            "[{}] Đang khởi động server Lữ Khách Ký Ức...",
            Local::now().format("%H:%M:%S")
        ));

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(500)).await;
                let keep = this.update(cx, |app, cx| {
                    app.poll_boot_and_refresh(cx);
                    cx.notify();
                });
                if keep.is_err() {
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
            users: Vec::new(),
            selected_account: None,
            online_only: false,
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

    fn poll_boot_and_refresh(&mut self, cx: &App) {
        if self.control.is_none()
            && self.boot_error.is_none()
            && let Ok(guard) = self.boot_state.lock()
        {
            match &*guard {
                BootState::Booting => {}
                BootState::Ready(ctrl) => {
                    let s = ctrl.summary();
                    self.control = Some(ctrl.clone());
                    self.push_feedback(format!("Server đã sẵn sàng tại {}", s.bind_addr));
                }
                BootState::Failed(err) => {
                    self.boot_error = Some(err.clone());
                    self.push_feedback(format!("Lỗi khởi động server: {err}"));
                }
            }
        }
        self.refresh_data(cx);
    }

    fn refresh_data(&mut self, cx: &App) {
        let Some(control) = &self.control else {
            return;
        };
        let query = self.search_input.read(cx).value().to_string();
        self.summary = Some(control.summary());
        self.users = control.list_users(&query, self.online_only);
        if let Some(sel) = self.selected_account
            && !self.users.iter().any(|u| u.account_id == sel)
        {
            self.selected_account = self.users.first().map(|u| u.account_id);
        } else if self.selected_account.is_none() {
            self.selected_account = self.users.first().map(|u| u.account_id);
        }
    }

    fn push_feedback(&mut self, msg: String) {
        let line = format!("[{}] {msg}", Local::now().format("%H:%M:%S"));
        if self.audit_logs.len() >= MAX_AUDIT_LOGS {
            self.audit_logs.pop_back();
        }
        self.audit_logs.push_front(line);
        self.status_banner = msg.into();
    }

    pub(crate) fn apply_result(&mut self, res: Result<String, String>, cx: &mut Context<Self>) {
        match res {
            Ok(msg) | Err(msg) => self.push_feedback(msg),
        }
        self.refresh_data(cx);
        cx.notify();
    }

    pub(crate) fn selected_user(&self) -> Option<&AdminUserRow> {
        let aid = self.selected_account?;
        self.users.iter().find(|u| u.account_id == aid)
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (maint, status_line) = match &self.summary {
            Some(s) => (
                s.maintaining,
                format!(
                    "Online: {} | Sessions: {} | Uptime: {}m{}s | {}",
                    s.online_players,
                    s.total_sessions,
                    s.uptime_secs / 60,
                    s.uptime_secs % 60,
                    if s.maintaining { "BẢO TRÌ" } else { "ONLINE" }
                ),
            ),
            None => (
                false,
                if self.boot_error.is_some() {
                    "LỖI KHỞI ĐỘNG".to_owned()
                } else {
                    "ĐANG KHỞI ĐỘNG...".to_owned()
                },
            ),
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(Label::new("Lữ Khách Ký Ức — Server Control Panel"))
                    .child(Label::new(status_line)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        Button::new("btn-refresh")
                            .label("Làm mới")
                            .icon(IconName::RefreshCw)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.poll_boot_and_refresh(cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("btn-maint")
                            .label(if maint { "Tắt bảo trì" } else { "Bật bảo trì" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(ctrl) = &this.control {
                                    let msg = ctrl.set_maintenance(!maint);
                                    this.push_feedback(msg);
                                    this.refresh_data(cx);
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        Button::new("btn-shutdown")
                            .danger()
                            .label("Dừng Server")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(ctrl) = &this.control {
                                    ctrl.trigger_shutdown();
                                }
                                this.push_feedback("Đang lưu dữ liệu và tắt server...".to_owned());
                                cx.quit();
                            })),
                    ),
            )
    }

    fn render_broadcast_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(div().flex_1().child(Input::new(&self.broadcast_input)))
            .child(
                Button::new("btn-broadcast")
                    .primary()
                    .label("Gửi thông báo")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let msg = this.broadcast_input.read(cx).value().to_string();
                        if let Some(ctrl) = &this.control {
                            let res = ctrl.broadcast(&msg);
                            this.apply_result(res, cx);
                        }
                    })),
            )
    }

    fn render_user_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let online_only = self.online_only;
        let mut rows_div = div().flex().flex_col().gap_1().flex_1().overflow_hidden();

        for user in self.users.iter().take(100) {
            let aid = user.account_id;
            let is_selected = self.selected_account == Some(aid);
            let status_txt = format!(
                "{}{}{}",
                if user.online { "ONLINE" } else { "OFF" },
                if user.active { " | TV" } else { "" },
                if user.ban { " | BAN" } else { "" }
            );
            let row_bg = if is_selected { theme.accent } else { theme.background };
            rows_div = rows_div.child(
                div()
                    .id(("user-row", aid.cast_unsigned() as usize))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(row_bg)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_account = Some(aid);
                        cx.notify();
                    }))
                    .child(Label::new(format!(
                        "#{aid} {} ({}) | Map {}:{}",
                        user.username, user.player_name, user.map_id, user.zone_id
                    )))
                    .child(Label::new(format!(
                        "Vàng: {} | Ngọc: {} | VND: {} | [{}]",
                        user.gold, user.gem, user.vnd, status_txt
                    ))),
            );
        }

        div()
            .flex()
            .flex_col()
            .flex_1()
            .p_3()
            .gap_2()
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.search_input)))
                    .child(
                        Button::new("btn-filter-online")
                            .label(if online_only { "Đang lọc: Online" } else { "Hiển thị: Tất cả" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.online_only = !this.online_only;
                                this.refresh_data(cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(Label::new(format!("Danh sách tài khoản ({}):", self.users.len())))
            .child(rows_div)
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut log_col = div().flex().flex_col().gap_1();
        for entry in self.audit_logs.iter().take(5) {
            log_col = log_col.child(Label::new(entry.clone()));
        }
        div()
            .flex()
            .flex_col()
            .px_4()
            .py_2()
            .gap_1()
            .border_t_1()
            .border_color(theme.border)
            .child(Label::new(format!("Trạng thái: {}", self.status_banner)))
            .child(log_col)
    }
}

impl Render for AdminApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let body = if let Some(err) = &self.boot_error {
            div()
                .flex()
                .flex_col()
                .flex_1()
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
                .overflow_hidden()
                .child(self.render_user_list(cx))
                .child(self.render_inspector(cx))
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_header(cx))
            .child(self.render_broadcast_bar(cx))
            .child(body)
            .child(self.render_footer(cx))
    }
}
