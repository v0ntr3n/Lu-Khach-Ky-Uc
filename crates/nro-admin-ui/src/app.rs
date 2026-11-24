//! Main GPUI Kit 3-tab dashboard & user control state for `nro-server-gui`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Local;
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::*;
use nro_server::admin_control::{AdminControl, AdminItemInfo, AdminUserRow, ServerSummary};

use crate::log_buffer::{LogBuffer, LogEntry};

pub(crate) const MAX_AUDIT_LOGS: usize = 60;

pub(crate) enum BootState {
    Booting,
    Ready(AdminControl),
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveTab {
    Users,
    ServerTools,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserFilter {
    Online,
    All,
    Banned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogLevelFilter {
    All,
    Info,
    Warn,
    Error,
}

pub(crate) fn format_num(n: i64) -> String {
    let neg = n < 0;
    let digits = n.unsigned_abs().to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3 + usize::from(neg));
    if neg {
        out.push('-');
    }
    for (idx, ch) in digits.chars().enumerate() {
        if idx > 0 && (len - idx).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

pub struct AdminApp {
    pub(crate) boot_state: Arc<Mutex<BootState>>,
    pub(crate) control: Option<AdminControl>,
    pub(crate) window_closed: Arc<AtomicBool>,
    pub(crate) boot_error: Option<String>,
    pub(crate) summary: Option<ServerSummary>,
    pub(crate) online_count: usize,
    pub(crate) bind_addr: String,
    pub(crate) active_tab: ActiveTab,
    pub(crate) user_filter: UserFilter,
    pub(crate) users: Vec<AdminUserRow>,
    pub(crate) selected_account: Option<i32>,
    pub(crate) is_refreshing: bool,
    pub(crate) refresh_pending: bool,
    pub(crate) action_in_flight: bool,
    pub(crate) shutting_down: bool,
    pub(crate) search_debounce_gen: u64,
    pub(crate) item_search_gen: u64,
    pub(crate) status_banner: SharedString,
    pub(crate) audit_log: Vec<(String, String)>,
    pub(crate) log_buffer: LogBuffer,
    pub(crate) logs: Vec<LogEntry>,
    pub(crate) last_log_seq: u64,
    pub(crate) log_warn_err_count: usize,
    pub(crate) log_level_filter: LogLevelFilter,
    pub(crate) log_paused: bool,
    pub(crate) search_input: Entity<InputState>,
    pub(crate) broadcast_input: Entity<InputState>,
    pub(crate) notify_input: Entity<InputState>,
    pub(crate) gold_input: Entity<InputState>,
    pub(crate) gem_input: Entity<InputState>,
    pub(crate) ruby_input: Entity<InputState>,
    pub(crate) vnd_input: Entity<InputState>,
    pub(crate) item_search_input: Entity<InputState>,
    pub(crate) item_results: Vec<AdminItemInfo>,
    pub(crate) selected_item: Option<(i16, String)>,
    pub(crate) item_id_input: Entity<InputState>,
    pub(crate) item_qty_input: Entity<InputState>,
    pub(crate) log_search_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl Drop for AdminApp {
    fn drop(&mut self) {
        self.shutting_down = true;
        if let Some(ctrl) = &self.control {
            ctrl.trigger_shutdown();
        } else if let Ok(guard) = self.boot_state.lock()
            && let BootState::Ready(ctrl) = &*guard
        {
            ctrl.trigger_shutdown();
        }
        self.window_closed.store(true, Ordering::Release);
    }
}

impl AdminApp {
    pub(crate) fn new(
        boot_state: Arc<Mutex<BootState>>,
        log_buffer: LogBuffer,
        window_closed: Arc<AtomicBool>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Tìm theo ID / username / tên nhân vật...")
        });
        let broadcast_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Nhập thông báo gửi toàn server..."));
        let notify_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Nhập thông báo riêng..."));
        let gold_input = cx.new(|cx| InputState::new(window, cx).placeholder("Vàng (+/-)"));
        let gem_input = cx.new(|cx| InputState::new(window, cx).placeholder("Ngọc (+/-)"));
        let ruby_input = cx.new(|cx| InputState::new(window, cx).placeholder("Hồng ngọc (+/-)"));
        let vnd_input = cx.new(|cx| InputState::new(window, cx).placeholder("VND (+/-)"));
        let item_search_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Tìm vật phẩm theo tên hoặc ID..."));
        let item_id_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Item Template ID"));
        let item_qty_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("SL (mặc định 1)"));
        let log_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Lọc log theo từ khóa..."));

        let sub_search = cx.subscribe_in(&search_input, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.schedule_debounced_refresh(cx);
            }
        });
        let sub_item = cx.subscribe_in(&item_search_input, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh_item_results(cx);
            }
        });
        let sub_log = cx.subscribe_in(&log_search_input, window, |_, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });

        let audit_log = vec![(
            Local::now().format("%H:%M:%S").to_string(),
            "Đang khởi động server Lữ Khách Ký Ức...".to_owned(),
        )];

        Self::spawn_user_poll(cx);
        Self::spawn_log_poll(cx);

        let mut app = Self {
            boot_state,
            control: None,
            window_closed,
            boot_error: None,
            summary: None,
            online_count: 0,
            bind_addr: String::new(),
            active_tab: ActiveTab::Users,
            user_filter: UserFilter::Online,
            users: Vec::new(),
            selected_account: None,
            is_refreshing: false,
            refresh_pending: false,
            action_in_flight: false,
            shutting_down: false,
            search_debounce_gen: 0,
            item_search_gen: 0,
            status_banner: "Đang khởi động server...".into(),
            audit_log,
            log_buffer,
            logs: Vec::new(),
            last_log_seq: 0,
            log_warn_err_count: 0,
            log_level_filter: LogLevelFilter::All,
            log_paused: false,
            search_input,
            broadcast_input,
            notify_input,
            gold_input,
            gem_input,
            ruby_input,
            vnd_input,
            item_search_input,
            item_results: Vec::new(),
            selected_item: None,
            item_id_input,
            item_qty_input,
            log_search_input,
            _subscriptions: vec![sub_search, sub_item, sub_log],
        };
        app.poll_boot_and_refresh(cx);
        app
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
            match self.active_tab {
                ActiveTab::Users => div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_user_list(cx))
                    .child(self.render_inspector(cx)),
                ActiveTab::ServerTools => div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_server_tools(cx)),
                ActiveTab::Logs => div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_log_view(cx)),
            }
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .bg(bg)
            .text_color(fg)
            .child(self.render_header(cx))
            .child(body)
            .child(self.render_footer(cx))
    }
}
