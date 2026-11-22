//! Inspector Currency/VND (Card 3), Item Grant/Search (Card 4), and input validation helpers.

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

fn parse_item_grant(
    id_raw: &str,
    qty_raw: &str,
    fallback_id: Option<i16>,
) -> Result<(i16, u32), String> {
    let id_trimmed = id_raw.trim();
    let tid = if id_trimmed.is_empty() {
        fallback_id.ok_or_else(|| "Vui lòng chọn vật phẩm hoặc nhập Item ID (>= 0).".to_owned())?
    } else {
        let parsed = id_trimmed
            .parse::<i16>()
            .map_err(|_| "Item Template ID không hợp lệ (cần số nguyên >= 0).".to_owned())?;
        if parsed < 0 {
            return Err("Item Template ID phải >= 0.".to_owned());
        }
        parsed
    };
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
    pub(crate) fn render_card_currency(
        &self,
        aid: i32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border = cx.theme().border;
        let busy = self.action_in_flight;

        div()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(border)
            .child(Label::new("Tiền tệ & VND:"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.gold_input)))
                    .child(div().flex_1().child(Input::new(&self.gem_input)))
                    .child(div().flex_1().child(Input::new(&self.ruby_input))),
            )
            .child(
                Button::new("btn-grant-money")
                    .primary()
                    .label(if busy {
                        "Đang xử lý..."
                    } else {
                        "Cộng/Trừ Vàng, Ngọc & Hồng ngọc"
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let gold_raw = this.gold_input.read(cx).value().to_string();
                        let gem_raw = this.gem_input.read(cx).value().to_string();
                        let ruby_raw = this.ruby_input.read(cx).value().to_string();
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
                        let ruby = match parse_optional_i32(&ruby_raw, "Hồng ngọc") {
                            Ok(v) => v,
                            Err(msg) => {
                                this.push_feedback(msg);
                                cx.notify();
                                return;
                            }
                        };
                        if gold == 0 && gem == 0 && ruby == 0 {
                            this.push_feedback(
                                "Vui lòng nhập ít nhất 1 giá trị tiền tệ khác 0.".to_owned(),
                            );
                            cx.notify();
                            return;
                        }
                        this.run_async_action(cx, move |ctrl| {
                            ctrl.grant_currency(aid, gold, gem, ruby)
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
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        Button::new("btn-p-100m")
                            .label("+100M Vàng")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.grant_currency(aid, 100_000_000, 0, 0)
                                });
                            })),
                    )
                    .child(
                        Button::new("btn-p-1b")
                            .label("+1B Vàng")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.grant_currency(aid, 1_000_000_000, 0, 0)
                                });
                            })),
                    )
                    .child(
                        Button::new("btn-p-10k-gem")
                            .label("+10K Ngọc")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.grant_currency(aid, 0, 10_000, 0)
                                });
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        Button::new("btn-p-100k-gem")
                            .label("+100K Ngọc")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.grant_currency(aid, 0, 100_000, 0)
                                });
                            })),
                    )
                    .child(
                        Button::new("btn-p-100k-vnd")
                            .label("+100K VND")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| ctrl.add_vnd(aid, 100_000));
                            })),
                    ),
            )
    }

    pub(crate) fn render_card_items(&self, aid: i32, cx: &mut Context<Self>) -> impl IntoElement {
        let (border, accent, bg) = (cx.theme().border, cx.theme().accent, cx.theme().background);
        let busy = self.action_in_flight;
        let selected_label = match &self.selected_item {
            Some((id, name)) => format!("Đã chọn: #{id} — {name}"),
            None => "Chưa chọn vật phẩm (chọn bên dưới hoặc nhập ID):".to_owned(),
        };

        let mut list = div().flex().flex_col().gap_1();
        for item in self.item_results.iter().take(6) {
            let item_id = item.id;
            let item_name = item.name.clone();
            let is_sel = self
                .selected_item
                .as_ref()
                .is_some_and(|(id, _)| *id == item_id);
            let row_bg = if is_sel { accent } else { bg };
            let tag = if item.stackable { "xN" } else { "x1" };
            list = list.child(
                div()
                    .id(("item-res", item_id.cast_unsigned() as usize))
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
                        this.selected_item = Some((item_id, item_name.clone()));
                        cx.notify();
                    }))
                    .child(Label::new(format!("#{item_id} — {}", item.name)))
                    .child(Label::new(tag)),
            );
        }

        div()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(border)
            .child(Label::new("Tặng Vật phẩm:"))
            .child(Input::new(&self.item_search_input))
            .child(list)
            .child(Label::new(selected_label))
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
                        let fallback = this.selected_item.as_ref().map(|(id, _)| *id);
                        let (tid, qty) = match parse_item_grant(&id_raw, &qty_raw, fallback) {
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
