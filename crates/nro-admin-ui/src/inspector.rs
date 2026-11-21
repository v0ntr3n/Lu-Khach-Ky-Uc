//! Right-hand user control inspector pane for `AdminApp`.

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::label::Label;
use gpui_kit::*;

use crate::app::AdminApp;

impl AdminApp {
    pub(crate) fn render_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let Some(user) = self.selected_user().cloned() else {
            return div()
                .w(px(380.0))
                .p_4()
                .child(Label::new("Chọn một tài khoản bên trái để điều khiển."));
        };
        let aid = user.account_id;
        let is_banned = user.ban;
        let is_active = user.active;

        div()
            .flex()
            .flex_col()
            .w(px(380.0))
            .p_4()
            .gap_3()
            .border_color(border)
            .child(Label::new(format!("Tài khoản #{aid} — {}", user.username)))
            .child(Label::new(format!(
                "Nhân vật: {} | SM: {} | Map {}:{}",
                user.player_name, user.power, user.map_id, user.zone_id
            )))
            .child(Label::new(format!(
                "Ví: {} vàng | {} ngọc | {} hồng ngọc | {} VND",
                user.gold, user.gem, user.ruby, user.vnd
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
                                this.run_async_action(cx, move |ctrl| ctrl.set_ban(aid, !is_banned));
                            })),
                    )
                    .child(
                        Button::new("btn-active")
                            .label(if is_active { "Hủy TV" } else { "Kích hoạt TV" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| ctrl.set_active(aid, !is_active));
                            })),
                    ),
            )
            .child(Label::new("Cập nhật Vàng / Ngọc / VND:"))
            .child(Input::new(&self.gold_input))
            .child(Input::new(&self.gem_input))
            .child(
                Button::new("btn-grant-money")
                    .primary()
                    .label("Cộng/Trừ Vàng & Ngọc")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let gold = this.gold_input.read(cx).value().trim().parse::<i64>().unwrap_or(0);
                        let gem = this.gem_input.read(cx).value().trim().parse::<i32>().unwrap_or(0);
                        this.run_async_action(cx, move |ctrl| ctrl.grant_currency(aid, gold, gem, 0));
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.vnd_input)))
                    .child(Button::new("btn-add-vnd").label("Cập nhật VND").on_click(
                        cx.listener(move |this, _, _, cx| {
                            let delta = this.vnd_input.read(cx).value().trim().parse::<i64>().unwrap_or(0);
                            this.run_async_action(cx, move |ctrl| ctrl.add_vnd(aid, delta));
                        }),
                    )),
            )
            .child(Label::new("Tặng vật phẩm vào hành trang:"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.item_id_input)))
                    .child(div().w(px(110.0)).child(Input::new(&self.item_qty_input))),
            )
            .child(
                Button::new("btn-give-item")
                    .primary()
                    .label("Tặng Item")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let tid = this.item_id_input.read(cx).value().trim().parse::<i16>().unwrap_or(-1);
                        let qty = this.item_qty_input.read(cx).value().trim().parse::<u32>().unwrap_or(1);
                        this.run_async_action(cx, move |ctrl| ctrl.give_item(aid, tid, qty));
                    })),
            )
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
                        this.run_async_action(cx, move |ctrl| ctrl.broadcast(&msg));
                    })),
            )
    }

}
