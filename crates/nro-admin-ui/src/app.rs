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
    is_refreshing: bool,
    pub(crate) status_banner: SharedString,
    pub(crate) audit_logs: VecDeque<String>,
    search_input: Entity<InputState>,
    pub(crate) broadcast_input: Entity<InputState>,
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
                this.spawn_refresh(cx);
            }
        });

        let mut audit_logs = VecDeque::with_capacity(MAX_AUDIT_LOGS);
        audit_logs.push_front(format!(
            "[{}] Đang khởi động server Lữ Khách Ký Ức...",
            Local::now().format("%H:%M:%S")
        ));

        cx.spawn(async move |this, cx| {
            loop {
                let is_booting = this
                    .read_with(cx, |app, _| app.control.is_none() && app.boot_error.is_none())
                    .unwrap_or(false);
                let delay = if is_booting { Duration::from_millis(500) } else { Duration::from_secs(3) };
                cx.background_executor().timer(delay).await;
                let keep = this.update(cx, |app, cx| {
                    app.poll_boot_and_refresh(cx);
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
            is_refreshing: false,
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

    fn poll_boot_and_refresh(&mut self, cx: &mut Context<Self>) {
        if self.control.is_none() && self.boot_error.is_none() {
            let next = self.boot_state.lock().ok().and_then(|guard| match &*guard {
                BootState::Booting => None,
                BootState::Ready(ctrl) => Some(Ok(ctrl.clone())),
                BootState::Failed(err) => Some(Err(err.clone())),
            });
            match next {
                Some(Ok(ctrl)) => {
                    let addr = ctrl.summary().bind_addr;
                    self.control = Some(ctrl);
                    self.push_feedback(format!("Server đã sẵn sàng tại {addr}"));
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

    pub(crate) fn spawn_refresh(&mut self, cx: &mut Context<Self>) {
        if self.is_refreshing {
            return;
        }
        let Some(control) = self.control.clone() else {
            return;
        };
        self.is_refreshing = true;
        let query = self.search_input.read(cx).value().to_string();
        let online_only = self.online_only;
        cx.spawn(async move |this, cx| {
            let (summary, users) = cx
                .background_executor()
                .spawn(async move { (control.summary(), control.list_users(&query, online_only)) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.summary = Some(summary);
                app.users = users;
                if let Some(sel) = app.selected_account
                    && !app.users.iter().any(|u| u.account_id == sel)
                {
                    app.selected_account = app.users.first().map(|u| u.account_id);
                } else if app.selected_account.is_none() {
                    app.selected_account = app.users.first().map(|u| u.account_id);
                }
                app.is_refreshing = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn run_async_action<F>(&mut self, cx: &mut Context<Self>, work: F)
    where
        F: FnOnce(AdminControl) -> Result<String, String> + Send + 'static,
    {
        let Some(control) = self.control.clone() else {
            self.push_feedback("Server chưa sẵn sàng.".to_owned());
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            let res = cx.background_executor().spawn(async move { work(control) }).await;
            let _ = this.update(cx, |app, cx| {
                match res {
                    Ok(msg) | Err(msg) => app.push_feedback(msg),
                }
                app.spawn_refresh(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn push_feedback(&mut self, msg: String) {
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

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
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
                if self.boot_error.is_some() { "LỖI KHỞI ĐỘNG".to_owned() } else { "ĐANG KHỞI ĐỘNG...".to_owned() },
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
            .border_color(border)
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
                            })),
                    )
                    .child(
                        Button::new("btn-maint")
                            .label(if maint { "Tắt bảo trì" } else { "Bật bảo trì" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| Ok(ctrl.set_maintenance(!maint)));
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

    fn render_user_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (border, accent, bg) = (cx.theme().border, cx.theme().accent, cx.theme().background);
        let online_only = self.online_only;
        let mut rows_div = div().flex().flex_col().gap_1().flex_1().overflow_hidden();

        for user in &self.users {
            let aid = user.account_id;
            let is_selected = self.selected_account == Some(aid);
            let status_txt = format!(
                "{}{}{}",
                if user.online { "ONLINE" } else { "OFF" },
                if user.active { " | TV" } else { "" },
                if user.ban { " | BAN" } else { "" }
            );
            let row_bg = if is_selected { accent } else { bg };
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
                    .border_color(border)
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
            .border_color(border)
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
                                this.spawn_refresh(cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(Label::new(format!("Danh sách tài khoản ({}):", self.users.len())))
            .child(rows_div)
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
            .bg(bg)
            .text_color(fg)
            .child(self.render_header(cx))
            .child(self.render_broadcast_bar(cx))
            .child(body)
            .child(self.render_footer(cx))
    }
}
