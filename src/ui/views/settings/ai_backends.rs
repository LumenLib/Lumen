use components::IconName;
use components::{muted_input, selector};
use gpui::prelude::*;
use gpui::{EntityInputHandler, SharedString, div, rems};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    setting::{SettingGroup, SettingItem, SettingPage},
    switch::Switch,
    v_flex,
};
use i18n::{I18nKey, t};
use services::app::MainApp;
use std::sync::Arc;
use translate;

use super::{SettingsWindow, lang};

impl SettingsWindow {
    pub(super) fn ai_backends_page(
        &self,
        _app: Arc<MainApp>,
        cx: &mut Context<Self>,
    ) -> SettingPage {
        let weak = cx.entity().downgrade();
        let l = lang(cx);

        SettingPage::new(t(I18nKey::AiBackendsSettingsTab, l))
            .icon(Icon::new(IconName::Puzzle))
            .group(
                SettingGroup::new()
                    .item(SettingItem::render({
                        let weak = weak.clone();
                        move |_, _window, cx| {
                            let theme = cx.theme();
                            let l = lang(cx);

                            let this = if let Some(t) = weak.upgrade() { t } else { return div().into_any_element(); };
                            let (
                                ai_entries,
                                ai_edit_target,
                                ai_adding_new,
                                ai_edit_enable_thinking,
                            ) = {
                                let t = this.read(cx);
                                (
                                    t.ai_entries.clone(),
                                    t.ai_edit_target,
                                    t.ai_adding_new,
                                    t.ai_edit_enable_thinking,
                                )
                            };
                            let (
                                ai_edit_name_input,
                                ai_edit_kind_value,
                                ai_edit_api_key_input,
                                ai_edit_api_base_input,
                                ai_edit_model_input,
                                ai_edit_context_window_input,
                                ai_edit_compression_strategy_value,
                            ) = {
                                let t = this.read(cx);
                                (
                                    t.ai_edit_name_input.clone(),
                                    t.ai_edit_kind_value.clone(),
                                    t.ai_edit_api_key_input.clone(),
                                    t.ai_edit_api_base_input.clone(),
                                    t.ai_edit_model_input.clone(),
                                    t.ai_edit_context_window_input.clone(),
                                    t.ai_edit_compression_strategy_value.clone(),
                                )
                            };

                            let mut list = v_flex().gap_2();

                            if !ai_adding_new && ai_edit_target.is_none() {
                                list = list.child(
                                    h_flex()
                                        .w_full()
                                        .justify_end()
                                        .child(
                                            Button::new("add-ai-backend")
                                                .label(t(I18nKey::AiAddBackend, l))
                                                .icon(IconName::Plus)
                                                .ghost()
                                                .small()
                                                .on_click({
                                                    let weak = weak.clone();
                                                    move |_, window, cx| {
                                                        if let Some(this) = weak.upgrade() {
                                                            this.update(cx, |t, cx| {
                                                                t.ai_adding_new = true;
                                                                t.ai_edit_target = None;
                                                                t.ai_edit_name_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), "", window, cx); });
                                                                t.ai_edit_api_key_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), "", window, cx); });
                                                                t.ai_edit_api_base_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), "", window, cx); });
                                                                t.ai_edit_model_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), "", window, cx); });
                                                                t.ai_edit_context_window_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), "4096", window, cx); });
                                                                t.ai_edit_enable_thinking = false;
                                                                cx.notify();
                                                            });
                                                        }
                                                    }
                                                }),
                                        ),
                                );

                                for (i, entry) in ai_entries.iter().enumerate() {
                                    let kind_display = match entry.kind.as_str() {
                                        "openai" => "OpenAI",
                                        "ollama" => "Ollama",
                                        "claude" => "Claude",
                                        "siliconflow" => "SiliconFlow",
                                        _ => &entry.kind,
                                    };

                                    list = list.child(
                                        div()
                                            .relative()
                                            .rounded_md()
                                            .bg(theme.muted)
                                            .p(rems(0.75))
                                            .child(
                                                h_flex()
                                                    .absolute()
                                                    .top_2()
                                                    .right_2()
                                                    .gap_1()
                                                    .child(
                                                        Button::new(SharedString::from(format!("edit-backend-{}", i)))
                                                            .label(t(I18nKey::Edit, l))
                                                            .ghost()
                                                            .small()
                                                            .on_click({
                                                                let weak = weak.clone();
                                                                let entry = entry.clone();
                                                                move |_, window, cx| {
                                                                    if let Some(t) = weak.upgrade() {
                                                                        t.update(cx, |t, cx| {
                                                                            t.ai_edit_target = Some(i);
                                                                            t.ai_adding_new = false;
                                                                            t.ai_edit_name_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), &entry.name, window, cx); });
                                                                            t.ai_edit_kind_value = entry.kind.clone().into();
                                                                            t.ai_edit_api_key_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), &entry.api_key, window, cx); });
                                                                            t.ai_edit_api_base_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), &entry.api_base, window, cx); });
                                                                            t.ai_edit_model_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), &entry.model, window, cx); });
                                                                            t.ai_edit_context_window_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), &entry.context_window.to_string(), window, cx); });
                                                                            t.ai_edit_compression_strategy_value = entry.compression_strategy.clone().into();
                                                                            t.ai_edit_enable_thinking = entry.enable_thinking;
                                                                            cx.notify();
                                                                        });
                                                                    }
                                                                }
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new(SharedString::from(format!("delete-backend-{}", i)))
                                                            .icon(IconName::Trash)
                                                            .ghost()
                                                            .small()
                                                            .text_color(theme.danger)
                                                            .on_click({
                                                                let weak = weak.clone();
                                                                move |_, _, cx| {
                                                                    if let Some(t) = weak.upgrade() {
                                                                        t.update(cx, |t, cx| {
                                                                            t.ai_entries.remove(i);
                                                                            if t.ai_edit_target == Some(i) {
                                                                                t.ai_edit_target = None;
                                                                            } else if let Some(edit_i) = t.ai_edit_target
                                                                                && edit_i > i {
                                                                                    t.ai_edit_target = Some(edit_i - 1);
                                                                                }
                                                                            cx.notify();
                                                                        });
                                                                    }
                                                                }
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                v_flex()
                                                    .gap_1()
                                                    .child(Label::new(entry.name.clone()).text_sm().font_weight(gpui::FontWeight::BOLD))
                                                    .child(
                                                        Label::new(format!("{} • {} • {}", kind_display, entry.model, entry.api_base))
                                                            .text_xs()
                                                            .text_color(theme.muted_foreground),
                                                    ),
                                            )
                                    );
                                }
                            } else {
                                // EDIT/ADD FORM
                                let title = if ai_adding_new {
                                    t(I18nKey::AiAddBackend, l).to_string()
                                } else {
                                    format!("{} {}", t(I18nKey::Edit, l), t(I18nKey::EngineAi, l))
                                };

                                list = list.child(
                                    v_flex()
                                        .gap_3()
                                        .child(Label::new(title).text_sm().font_weight(gpui::FontWeight::BOLD))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiBackendName, l)).text_sm().w(rems(6.0))).child(muted_input(&ai_edit_name_input, theme).flex_grow(1.0)))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiBackendType, l)).text_sm().w(rems(6.0))).child({
                                            let weak = weak.clone();
                                            selector(
                                                "ai-edit-kind",
                                                vec![
                                                    ("openai".into(), "OpenAI".into()),
                                                    ("ollama".into(), "Ollama".into()),
                                                    ("claude".into(), "Claude".into()),
                                                    ("siliconflow".into(), "SiliconFlow".into()),
                                                ],
                                                ai_edit_kind_value.clone(),
                                                false,
                                                move |v, _, cx| {
                                                    if let Some(this) = weak.upgrade() {
                                                        this.update(cx, |this, _| {
                                                            this.ai_edit_kind_value = v;
                                                        });
                                                    }
                                                },
                                            )
                                        }))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiApiKey, l)).text_sm().w(rems(6.0))).child(muted_input(&ai_edit_api_key_input, theme).flex_grow(1.0)))
                                        .when(
                                            ai_edit_kind_value.as_ref() != "siliconflow",
                                            |this| this.child(h_flex().gap_2().child(Label::new(t(I18nKey::AiApiBase, l)).text_sm().w(rems(6.0))).child(muted_input(&ai_edit_api_base_input, theme).flex_grow(1.0)))
                                        )
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiModel, l)).text_sm().w(rems(6.0))).child(muted_input(&ai_edit_model_input, theme).flex_grow(1.0)))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiContextWindow, l)).text_sm().w(rems(6.0))).child(muted_input(&ai_edit_context_window_input, theme).flex_grow(1.0)))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiCompressionStrategy, l)).text_sm().w(rems(6.0))).child({
                                            let weak = weak.clone();
                                            selector(
                                                "ai-edit-compression",
                                                vec![
                                                    ("none".into(), "None".into()),
                                                    ("summary".into(), "Summary".into()),
                                                ],
                                                ai_edit_compression_strategy_value.clone(),
                                                false,
                                                move |v, _, cx| {
                                                    if let Some(this) = weak.upgrade() {
                                                        this.update(cx, |this, _| {
                                                            this.ai_edit_compression_strategy_value = v;
                                                        });
                                                    }
                                                },
                                            )
                                        }))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::EnableThinking, l)).text_sm().w(rems(6.0))).child(Switch::new("enable-thinking").checked(ai_edit_enable_thinking).on_click({
                                            let weak = weak.clone();
                                            move |v: &bool, _, cx| {
                                                if let Some(this) = weak.upgrade() {
                                                    this.update(cx, |t, cx| { t.ai_edit_enable_thinking = *v; cx.notify(); });
                                                }
                                            }
                                        })))
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .justify_end()
                                                .child(
                                                    Button::new("cancel-edit-ai")
                                                        .ghost()
                                                        .icon(IconName::Close)
                                                        .tooltip(t(I18nKey::Cancel, l))
                                                        .on_click({
                                                            let weak = weak.clone();
                                                            move |_, _, cx| {
                                                                if let Some(this) = weak.upgrade() {
                                                                    this.update(cx, |t, cx| {
                                                                        t.ai_adding_new = false;
                                                                        t.ai_edit_target = None;
                                                                        cx.notify();
                                                                    });
                                                                }
                                                            }
                                                        })
                                                )
                                                .child(
                                                    Button::new("save-edit-ai")
                                                        .primary()
                                                        .icon(IconName::Check)
                                                        .tooltip(t(I18nKey::Save, l))
                                                        .on_click({
                                                            let weak = weak.clone();
                                                            move |_, _, cx| {
                                                                if let Some(this) = weak.upgrade() {
                                                                    this.update(cx, |t, cx| {
                                                                        let new_entry = translate::AiBackendEntry {
                                                                            name: t.ai_edit_name_input.read(cx).text().to_string(),
                                                                            kind: t.ai_edit_kind_value.to_string(),
                                                                            api_key: t.ai_edit_api_key_input.read(cx).text().to_string(),
                                                                            api_base: t.ai_edit_api_base_input.read(cx).text().to_string(),
                                                                            model: t.ai_edit_model_input.read(cx).text().to_string(),
                                                                            context_window: t.ai_edit_context_window_input.read(cx).text().to_string().parse().unwrap_or(4096),
                                                                            compression_strategy: t.ai_edit_compression_strategy_value.to_string(),
                                                                            enable_thinking: t.ai_edit_enable_thinking,
                                                                            max_tokens: 4096,
                                                                            temperature: 0.3,
                                                                        };

                                                                        if t.ai_adding_new {
                                                                            t.ai_entries.push(new_entry);
                                                                        } else if let Some(idx) = t.ai_edit_target
                                                                            && idx < t.ai_entries.len() {
                                                                                t.ai_entries[idx] = new_entry;
                                                                            }

                                                                        t.ai_adding_new = false;
                                                                        t.ai_edit_target = None;
                                                                        cx.notify();
                                                                    });
                                                                }
                                                            }
                                                        }),
                                                ),
                                        )
                                );
                            }

                            list.into_any_element()
                        }
                    })),
            )
    }
}
