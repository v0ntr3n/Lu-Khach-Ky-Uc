//! Header, broadcast bar, scrollable user list, and audit footer views for `AdminApp`.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::label::Label;
use gpui_kit::component::{ActiveTheme, IconName};
use gpui_kit::*;

use crate::app::AdminApp;

impl AdminApp {
    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let (maint, status_line) = match &self.summary {
            Some(s) => (
                s.maintaining,
                format!(
                    "{} | Online: {} | Sessions: {} | Uptime: {}m{}s | {}",
                    self.bind_addr,
                    self.online_count,
                    s.total_sessions,
                    s.uptime_secs / 60,
                    s.uptime_secs % 60,
                    if s.maintaining {
                        "BẢO TRÌ"
                    } else {
                        "ONLINE"
                    }
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
                            .label(if maint {
                                "Tắt bảo trì"
                            } else {
                                "Bật bảo trì"
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    Ok(ctrl.set_maintenance(!maint))
                                });
                            })),
                    )
                    .child(
                        Button::new("btn-shutdown")
                            .danger()
                            .label("Dừng Server")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request_shutdown(cx);
                            })),
                    ),
            )
    }

    pub(crate) fn render_user_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (border, accent, bg) = (cx.theme().border, cx.theme().accent, cx.theme().background);
        let show_offline = self.show_offline;
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
                        Button::new("btn-filter-online")
                            .label(if show_offline { "All" } else { "Online" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_offline = !this.show_offline;
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
            .border_color(border)
            .child(Label::new(format!("Trạng thái: {}", self.status_banner)))
            .child(log_col)
    }

    pub(crate) fn render_broadcast_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(border)
            .child(div().flex_1().child(Input::new(&self.broadcast_input)))
            .child(
                Button::new("btn-broadcast")
                    .primary()
                    .label("Gửi thông báo")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let msg = this.broadcast_input.read(cx).value().to_string();
                        if msg.trim().is_empty() {
                            this.push_feedback(
                                "Nội dung thông báo không được để trống.".to_owned(),
                            );
                            cx.notify();
                            return;
                        }
                        this.run_async_action(cx, move |ctrl| ctrl.broadcast(&msg));
                    })),
            )
    }
}
