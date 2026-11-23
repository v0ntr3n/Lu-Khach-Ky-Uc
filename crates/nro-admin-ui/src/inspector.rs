//! Right-hand card-grouped user control inspector pane (Profile & Private Notify cards).

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::label::Label;
use gpui_kit::*;
use nro_server::admin_control::AdminUserRow;

use crate::app::{AdminApp, format_num};

impl AdminApp {
    pub(crate) fn render_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(user) = self.selected_user().cloned() else {
            return div()
                .id("inspector-scroll")
                .flex()
                .flex_col()
                .w(px(420.0))
                .min_h_0()
                .overflow_y_scroll()
                .p_4()
                .child(Label::new("Chọn một tài khoản bên trái để điều khiển."));
        };

        div()
            .id("inspector-scroll")
            .flex()
            .flex_col()
            .w(px(420.0))
            .min_h_0()
            .overflow_y_scroll()
            .p_3()
            .gap_3()
            .child(self.render_card_profile(&user, cx))
            .child(self.render_card_pet(&user, cx))
            .child(self.render_card_notify(user.account_id, cx))
            .child(self.render_card_currency(user.account_id, cx))
            .child(self.render_card_items(user.account_id, cx))
    }

    fn render_card_profile(&self, user: &AdminUserRow, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let aid = user.account_id;
        let is_banned = user.ban;
        let is_active = user.active;

        div()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(border)
            .child(Label::new(format!("Tài khoản #{aid} — {}", user.username)))
            .child(Label::new(format!(
                "Nhân vật: {} | SM: {} | Map {}:{} ({})",
                user.player_name,
                format_num(user.power),
                user.map_id,
                user.zone_id,
                user.map_name
            )))
            .child(Label::new(format!(
                "Ví: {} vàng | {} ngọc | {} hồng ngọc | {} VND",
                format_num(user.gold),
                format_num(i64::from(user.gem)),
                format_num(i64::from(user.ruby)),
                format_num(i64::from(user.vnd)),
            )))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("btn-kick").label("Kick").on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.run_async_action(cx, move |ctrl| ctrl.kick_user(aid));
                        },
                    )))
                    .child(
                        Button::new("btn-ban")
                            .danger()
                            .label(if is_banned { "Unban" } else { "Ban" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.set_ban(aid, !is_banned)
                                });
                            })),
                    )
                    .child(
                        Button::new("btn-active")
                            .label(if is_active {
                                "Hủy TV"
                            } else {
                                "Kích hoạt TV"
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.set_active(aid, !is_active)
                                });
                            })),
                    )
                    .child(
                        Button::new("btn-heal")
                            .label("Hồi HP/MP")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| ctrl.heal_player(aid));
                            })),
                    ),
            )
    }

    fn render_card_notify(&self, aid: i32, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        div()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(border)
            .child(Label::new("Thông báo riêng:"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.notify_input)))
                    .child(
                        Button::new("btn-notify-user")
                            .primary()
                            .label("Gửi")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let msg = this.notify_input.read(cx).value().to_string();
                                if msg.trim().is_empty() {
                                    this.push_feedback(
                                        "Nội dung thông báo riêng không được để trống.".to_owned(),
                                    );
                                    cx.notify();
                                    return;
                                }
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.notify_player(aid, &msg)
                                });
                            })),
                    ),
            )
    }
}
