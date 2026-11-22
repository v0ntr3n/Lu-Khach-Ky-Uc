//! Header with KPI bar, 3-tab navigation, scrollable user list, and status footer for `AdminApp`.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::label::Label;
use gpui_kit::component::{ActiveTheme, IconName};
use gpui_kit::*;

use crate::app::{ActiveTab, AdminApp, UserFilter, format_num};

fn tab_button(id: &'static str, label: String, active: bool) -> Button {
    let btn = Button::new(id).label(label);
    if active { btn.primary() } else { btn }
}

fn filter_button(id: &'static str, label: &'static str, active: bool) -> Button {
    let btn = Button::new(id).label(label);
    if active { btn.primary() } else { btn }
}

impl AdminApp {
    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let status_line = match &self.summary {
            Some(s) => format!(
                "{} | Online: {} | Sessions: {} | Uptime: {}m{}s | {}",
                if s.maintaining {
                    "BẢO TRÌ"
                } else {
                    "ONLINE"
                },
                self.online_count,
                s.total_sessions,
                s.uptime_secs / 60,
                s.uptime_secs % 60,
                self.bind_addr,
            ),
            None => {
                if self.boot_error.is_some() {
                    "LỖI KHỞI ĐỘNG".to_owned()
                } else {
                    "ĐANG KHỞI ĐỘNG...".to_owned()
                }
            }
        };

        let logs_label = if self.log_warn_err_count > 0 {
            format!("Server Logs ({})", self.log_warn_err_count)
        } else {
            "Server Logs".to_owned()
        };

        div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .py_3()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(Label::new("Lữ Khách Ký Ức — Admin Control"))
                            .child(Label::new(status_line)),
                    )
                    .child(
                        Button::new("btn-refresh")
                            .label("Làm mới")
                            .icon(IconName::RefreshCw)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.poll_boot_and_refresh(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(border)
                    .child(
                        tab_button(
                            "tab-users",
                            "Người chơi".to_owned(),
                            self.active_tab == ActiveTab::Users,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.active_tab = ActiveTab::Users;
                            cx.notify();
                        })),
                    )
                    .child(
                        tab_button(
                            "tab-tools",
                            "Công cụ Server".to_owned(),
                            self.active_tab == ActiveTab::ServerTools,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.active_tab = ActiveTab::ServerTools;
                            cx.notify();
                        })),
                    )
                    .child(
                        tab_button("tab-logs", logs_label, self.active_tab == ActiveTab::Logs)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_tab = ActiveTab::Logs;
                                cx.notify();
                            })),
                    ),
            )
    }

    pub(crate) fn render_user_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (border, accent, bg) = (cx.theme().border, cx.theme().accent, cx.theme().background);
        let mut rows_div = div()
            .id("user-list-scroll")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();

        for user in &self.users {
            let aid = user.account_id;
            let is_selected = self.selected_account == Some(aid);
            let status_txt = format!(
                "{}{}{}{}",
                if user.online { "ONLINE" } else { "OFF" },
                if user.active { " | TV" } else { "" },
                if user.ban { " | BAN" } else { "" },
                if user.is_admin { " | ADMIN" } else { "" },
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
                        "#{aid} {} ({}) | Map {}:{} ({})",
                        user.username, user.player_name, user.map_id, user.zone_id, user.map_name
                    )))
                    .child(Label::new(format!(
                        "Vàng: {} | Ngọc: {} | [{}]",
                        format_num(user.gold),
                        format_num(i64::from(user.gem)),
                        status_txt
                    ))),
            );
        }

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
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
                        filter_button(
                            "btn-filter-online",
                            "Online",
                            self.user_filter == UserFilter::Online,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.user_filter = UserFilter::Online;
                            this.spawn_refresh(cx);
                            cx.notify();
                        })),
                    )
                    .child(
                        filter_button(
                            "btn-filter-all",
                            "Tất cả",
                            self.user_filter == UserFilter::All,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.user_filter = UserFilter::All;
                            this.spawn_refresh(cx);
                            cx.notify();
                        })),
                    )
                    .child(
                        filter_button(
                            "btn-filter-banned",
                            "Bị Ban",
                            self.user_filter == UserFilter::Banned,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.user_filter = UserFilter::Banned;
                            this.spawn_refresh(cx);
                            cx.notify();
                        })),
                    ),
            )
            .child(Label::new(format!(
                "Danh sách tài khoản ({}):",
                self.users.len()
            )))
            .child(rows_div)
    }

    pub(crate) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        div()
            .flex()
            .flex_row()
            .items_center()
            .px_4()
            .py_2()
            .border_t_1()
            .border_color(border)
            .child(Label::new(format!("Trạng thái: {}", self.status_banner)))
    }
}
