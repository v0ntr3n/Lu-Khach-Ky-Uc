//! Right-hand user control inspector pane and input validation for `AdminApp`.

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::label::Label;
use gpui_kit::*;

use crate::app::AdminApp;

fn parse_optional_i64(raw: &str, field: &str) -> Result<i64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    trimmed
        .parse::<i64>()
        .map_err(|_| format!("Giá trị {field} không hợp lệ (cần số nguyên)."))
}

fn parse_optional_i32(raw: &str, field: &str) -> Result<i32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    trimmed
        .parse::<i32>()
        .map_err(|_| format!("Giá trị {field} không hợp lệ (cần số nguyên)."))
}

fn parse_required_nonzero_i64(raw: &str, field: &str) -> Result<i64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("Vui lòng nhập {field} khác 0."));
    }
    let val = trimmed
        .parse::<i64>()
        .map_err(|_| format!("Giá trị {field} không hợp lệ (cần số nguyên khác 0)."))?;
    if val == 0 {
        return Err(format!("Giá trị {field} phải khác 0."));
    }
    Ok(val)
}

fn parse_item_grant(id_raw: &str, qty_raw: &str) -> Result<(i16, u32), String> {
    let id_trimmed = id_raw.trim();
    if id_trimmed.is_empty() {
        return Err("Vui lòng nhập Item Template ID (>= 0).".to_owned());
    }
    let tid = id_trimmed
        .parse::<i16>()
        .map_err(|_| "Item Template ID không hợp lệ (cần số nguyên >= 0).".to_owned())?;
    if tid < 0 {
        return Err("Item Template ID phải >= 0.".to_owned());
    }
    let qty_trimmed = qty_raw.trim();
    let qty = if qty_trimmed.is_empty() {
        1
    } else {
        let parsed = qty_trimmed
            .parse::<u32>()
            .map_err(|_| "Số lượng vật phẩm không hợp lệ (cần số nguyên dương > 0).".to_owned())?;
        if parsed == 0 {
            return Err("Số lượng vật phẩm phải > 0.".to_owned());
        }
        parsed
    };
    Ok((tid, qty))
}

impl AdminApp {
    pub(crate) fn render_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let Some(user) = self.selected_user().cloned() else {
            return div()
                .id("inspector-scroll")
                .flex()
                .flex_col()
                .w(px(380.0))
                .min_h_0()
                .overflow_y_scroll()
                .p_4()
                .child(Label::new("Chọn một tài khoản bên trái để điều khiển."));
        };
        let aid = user.account_id;
        let is_banned = user.ban;
        let is_active = user.active;
        let busy = self.action_in_flight;

        div()
            .id("inspector-scroll")
            .flex()
            .flex_col()
            .w(px(380.0))
            .min_h_0()
            .overflow_y_scroll()
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
                    ),
            )
            .child(Label::new("Cập nhật Vàng / Ngọc / VND:"))
            .child(Input::new(&self.gold_input))
            .child(Input::new(&self.gem_input))
            .child(
                Button::new("btn-grant-money")
                    .primary()
                    .label(if busy {
                        "Đang xử lý..."
                    } else {
                        "Cộng/Trừ Vàng & Ngọc"
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let gold_raw = this.gold_input.read(cx).value().to_string();
                        let gem_raw = this.gem_input.read(cx).value().to_string();
                        let gold = match parse_optional_i64(&gold_raw, "Vàng") {
                            Ok(v) => v,
                            Err(msg) => {
                                this.push_feedback(msg);
                                cx.notify();
                                return;
                            }
                        };
                        let gem = match parse_optional_i32(&gem_raw, "Ngọc") {
                            Ok(v) => v,
                            Err(msg) => {
                                this.push_feedback(msg);
                                cx.notify();
                                return;
                            }
                        };
                        if gold == 0 && gem == 0 {
                            this.push_feedback(
                                "Vui lòng nhập ít nhất 1 giá trị Vàng hoặc Ngọc khác 0.".to_owned(),
                            );
                            cx.notify();
                            return;
                        }
                        this.run_async_action(cx, move |ctrl| {
                            ctrl.grant_currency(aid, gold, gem, 0)
                        });
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.vnd_input)))
                    .child(
                        Button::new("btn-add-vnd")
                            .label("Cập nhật VND")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let vnd_raw = this.vnd_input.read(cx).value().to_string();
                                let delta = match parse_required_nonzero_i64(&vnd_raw, "VND") {
                                    Ok(v) => v,
                                    Err(msg) => {
                                        this.push_feedback(msg);
                                        cx.notify();
                                        return;
                                    }
                                };
                                this.run_async_action(cx, move |ctrl| ctrl.add_vnd(aid, delta));
                            })),
                    ),
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
                    .label(if busy {
                        "Đang xử lý..."
                    } else {
                        "Tặng Item"
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let id_raw = this.item_id_input.read(cx).value().to_string();
                        let qty_raw = this.item_qty_input.read(cx).value().to_string();
                        let (tid, qty) = match parse_item_grant(&id_raw, &qty_raw) {
                            Ok(v) => v,
                            Err(msg) => {
                                this.push_feedback(msg);
                                cx.notify();
                                return;
                            }
                        };
                        this.run_async_action(cx, move |ctrl| ctrl.give_item(aid, tid, qty));
                    })),
            )
    }
}
