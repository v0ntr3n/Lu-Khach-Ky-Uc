//! Inspector Pet (`Đệ tử`) card: status badge, 1-click pet type grant/switch, and SM milestones.

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::*;
use nro_server::admin_control::{AdminUserRow, pet_type_label};

use crate::app::{AdminApp, format_num};

const PET_POWER_150M: i64 = 150_000_000;
const PET_POWER_1_5B: i64 = 1_500_000_000;
const PET_POWER_20B: i64 = 20_000_000_000;
const PET_POWER_40B: i64 = 40_000_000_000;

fn pet_type_btn(id: &'static str, label: &'static str, active: bool) -> Button {
    let btn = Button::new(id).label(label);
    if active { btn.primary() } else { btn }
}

impl AdminApp {
    pub(crate) fn render_card_pet(
        &self,
        user: &AdminUserRow,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (border, accent, bg) = (cx.theme().border, cx.theme().accent, cx.theme().background);
        let aid = user.account_id;
        let busy = self.action_in_flight;
        let cur_type = user.has_pet.then_some(user.pet_type);

        let card_title = if busy {
            "Quản lý Đệ tử (Pet) — Đang xử lý..."
        } else {
            "Quản lý Đệ tử (Pet)"
        };

        let badge_text = if user.has_pet {
            let pet_name = if user.pet_name.is_empty() {
                "Đệ tử"
            } else {
                user.pet_name.as_str()
            };
            format!(
                "{pet_name} (Loại: {}) | SM: {}",
                pet_type_label(user.pet_type),
                format_num(user.pet_power),
            )
        } else {
            "Chưa có Đệ tử".to_owned()
        };
        let badge_bg = if user.has_pet { accent } else { bg };

        div()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(border)
            .child(Label::new(card_title))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(badge_bg)
                    .child(Label::new(badge_text)),
            )
            .child(Label::new("Cấp / Đổi loại Đệ tử:"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        pet_type_btn("btn-pet-type-0", "Tặng Đệ Thường", cur_type == Some(0))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.grant_or_set_pet(aid, 0)
                                });
                            })),
                    )
                    .child(
                        pet_type_btn("btn-pet-type-1", "Đệ Mabư", cur_type == Some(1)).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.grant_or_set_pet(aid, 1)
                                });
                            }),
                        ),
                    )
                    .child(
                        pet_type_btn("btn-pet-type-2", "Đệ Uub", cur_type == Some(2)).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.grant_or_set_pet(aid, 2)
                                });
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        pet_type_btn("btn-pet-type-3", "Đệ Kid Beer", cur_type == Some(3))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.grant_or_set_pet(aid, 3)
                                });
                            })),
                    )
                    .child(
                        pet_type_btn("btn-pet-type-4", "Đệ Kid Jiren", cur_type == Some(4))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.grant_or_set_pet(aid, 4)
                                });
                            })),
                    ),
            )
            .child(Label::new("Cộng Sức mạnh Đệ tử (mở kỹ năng):"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("btn-pet-sm-150m").label("+150 Tr SM").on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.run_async_action(cx, move |ctrl| {
                                ctrl.add_pet_power(aid, PET_POWER_150M)
                            });
                        }),
                    ))
                    .child(
                        Button::new("btn-pet-sm-1500m")
                            .label("+1.5 Tỷ SM")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.add_pet_power(aid, PET_POWER_1_5B)
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
                        Button::new("btn-pet-sm-20b")
                            .label("+20 Tỷ SM")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.add_pet_power(aid, PET_POWER_20B)
                                });
                            })),
                    )
                    .child(
                        Button::new("btn-pet-sm-40b")
                            .label("+40 Tỷ SM")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_async_action(cx, move |ctrl| {
                                    ctrl.add_pet_power(aid, PET_POWER_40B)
                                });
                            })),
                    ),
            )
    }
}
