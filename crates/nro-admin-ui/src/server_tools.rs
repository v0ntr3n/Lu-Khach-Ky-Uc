//! Server Tools tab (Broadcast presets, Server Operations, and Admin Audit History).

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::label::Label;
use gpui_kit::*;

use crate::app::AdminApp;

const PRESET_MAINT: &str = "Server sắp bảo trì sau 5 phút, vui lòng thoát game an toàn!";
const PRESET_WELCOME: &str = "Chào mừng đến với Lữ Khách Ký Ức!";
const PRESET_EVENT: &str = "Sự kiện x2 tiềm năng đã bắt đầu!";

impl AdminApp {
    pub(crate) fn render_server_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let maint = self.summary.as_ref().is_some_and(|s| s.maintaining);

        let mut audit_list = div().flex().flex_col().gap_1();

        if self.audit_log.is_empty() {
            audit_list = audit_list.child(Label::new("Chưa có thao tác admin nào."));
        } else {
            for (ts, msg) in &self.audit_log {
                audit_list = audit_list.child(Label::new(format!("[{ts}] {msg}")));
            }
        }

        div()
            .id("server-tools-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_4()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p_4()
                    .gap_2()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .child(Label::new("Thông báo toàn Server (Broadcast)"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.broadcast_input)))
                            .child(
                                Button::new("btn-broadcast")
                                    .primary()
                                    .label("Gửi thông báo")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let msg = this.broadcast_input.read(cx).value().to_string();
                                        if msg.trim().is_empty() {
                                            this.push_feedback(
                                                "Nội dung thông báo không được để trống."
                                                    .to_owned(),
                                            );
                                            cx.notify();
                                            return;
                                        }
                                        this.run_async_action(cx, move |ctrl| ctrl.broadcast(&msg));
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                Button::new("btn-preset-maint")
                                    .label("Bảo trì 5 phút")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.run_async_action(cx, |ctrl| {
                                            ctrl.broadcast(PRESET_MAINT)
                                        });
                                    })),
                            )
                            .child(
                                Button::new("btn-preset-welcome")
                                    .label("Chào mừng")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.run_async_action(cx, |ctrl| {
                                            ctrl.broadcast(PRESET_WELCOME)
                                        });
                                    })),
                            )
                            .child(
                                Button::new("btn-preset-event")
                                    .label("Sự kiện x2")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.run_async_action(cx, |ctrl| {
                                            ctrl.broadcast(PRESET_EVENT)
                                        });
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p_4()
                    .gap_2()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .child(Label::new("Vận hành Server"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                Button::new("btn-save-all")
                                    .primary()
                                    .label("Lưu tất cả (Save All)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.run_async_action(cx, |ctrl| ctrl.save_all_online());
                                    })),
                            )
                            .child(
                                Button::new("btn-kick-all")
                                    .danger()
                                    .label("Kick tất cả (Kick All)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.run_async_action(cx, |ctrl| ctrl.kick_all_online());
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
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p_4()
                    .gap_2()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .child(Label::new(format!(
                        "Lịch sử thao tác Admin ({}):",
                        self.audit_log.len()
                    )))
                    .child(audit_list),
            )
    }
}
