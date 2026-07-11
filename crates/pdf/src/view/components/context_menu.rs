use crate::annotation::ToolbarAnnotationKind;
use crate::view::{PAGE_BASE_WIDTH_REMS, PdfIconName, PdfReaderView, TOOLBAR_HEIGHT_REMS, helpers};
use gpui::prelude::*;
use gpui::{
    AnyElement, AppContext, Context, MouseButton, PathPromptOptions, Pixels, SharedString, Window,
    anchored, div, px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::{
    ActiveTheme, Icon, h_flex,
    menu::{PopupMenu, PopupMenuItem},
    v_flex,
};
use i18n::I18nKey;
use models::AnnotationColor;

impl PdfReaderView {
    fn find_annotation(&self, id: &str) -> Option<crate::Annotation> {
        for annotations in self.annotation_state.annotations.values() {
            for ann in annotations {
                if !ann.is_deleted && ann.id == id {
                    return Some(ann.clone());
                }
            }
        }
        None
    }

    fn find_annotation_mut(&mut self, id: &str) -> Option<&mut crate::Annotation> {
        for annotations in self.annotation_state.annotations.values_mut() {
            for ann in annotations.iter_mut() {
                if !ann.is_deleted && ann.id == id {
                    return Some(ann);
                }
            }
        }
        None
    }

    fn update_and_save(&mut self, id: &str, f: impl FnOnce(&mut crate::Annotation)) {
        let cloned = match self.find_annotation_mut(id) {
            Some(ann) => {
                f(ann);
                ann.clone()
            }
            None => return,
        };
        if let Some(delegate) = &self.delegate {
            delegate.save_annotation(&cloned);
        }
    }

    pub(crate) fn render_annotation_context_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let ctx_state = self.annotation_state.context_menu.as_ref()?;
        let ann = self.find_annotation(&ctx_state.annotation_id)?;

        let adjusted_pos = self.adjust_context_menu_position(ctx_state.position, window);
        let this_weak = cx.weak_entity();
        let lang = self.language;

        let has_note = ann
            .note
            .as_ref()
            .map(|n| !n.trim().is_empty())
            .unwrap_or(false);
        let ann_id_note = ctx_state.annotation_id.clone();
        let ann_id_delete = ctx_state.annotation_id.clone();
        let ann_id_color = ctx_state.annotation_id.clone();
        let current_color = ann.color;
        let this_weak_color = this_weak.clone();

        let is_text = matches!(
            ann.kind,
            crate::AnnotationKind::Highlight | crate::AnnotationKind::Underline
        );

        let this_weak_type = this_weak.clone();
        let ann_id_type = ctx_state.annotation_id.clone();
        let current_kind = ann.kind.clone();

        let rect_coords = match &ann.kind {
            crate::AnnotationKind::Rectangle { x, y, w, h } => Some((*x, *y, *w, *h)),
            _ => None,
        };

        let this_weak_copy = this_weak.clone();
        let ann_page = ann.page;
        let this_weak_save = this_weak.clone();
        let this_weak_pip = this_weak.clone();
        let this_weak_note = this_weak.clone();
        let this_weak_delete = this_weak.clone();

        let colors = vec![
            AnnotationColor::Yellow,
            AnnotationColor::Red,
            AnnotationColor::Green,
            AnnotationColor::Blue,
            AnnotationColor::Purple,
            AnnotationColor::Magenta,
            AnnotationColor::Orange,
            AnnotationColor::Gray,
        ];

        let menu = PopupMenu::build(window, cx, move |mut menu, _window, _cx| {
            // 1. 颜色选择小圆点
            let this_weak_c = this_weak_color.clone();
            let ann_id_c = ann_id_color.clone();
            menu = menu.item(PopupMenuItem::element(move |_window, _cx| {
                let this_weak = this_weak_c.clone();
                let ann_id = ann_id_c.clone();
                let current = current_color;

                h_flex()
                    .ml(gpui::px(-16.0)) // 抵消系统组件默认空 Icon 占位间隙
                    .w_full()
                    .justify_around()
                    .py_1()
                    .children(
                        colors
                            .iter()
                            .map(move |&color| {
                                let hex = color.to_hex();
                                let color_val =
                                    u32::from_str_radix(&hex[1..], 16).unwrap_or(0x000000);
                                let r = ((color_val >> 16) & 0xFF) as f32 / 255.0;
                                let g = ((color_val >> 8) & 0xFF) as f32 / 255.0;
                                let b = (color_val & 0xFF) as f32 / 255.0;
                                let hsla = gpui::Hsla::from(gpui::Rgba { r, g, b, a: 1.0 });

                                let is_active = current == color;
                                let this_weak_click = this_weak.clone();
                                let ann_id = ann_id.clone();

                                div()
                                    .id(SharedString::from(format!("color_dot_{:?}", color)))
                                    .size_4()
                                    .rounded_full()
                                    .bg(hsla)
                                    .cursor_pointer()
                                    .border_2()
                                    .border_color(if is_active {
                                        gpui::white()
                                    } else {
                                        gpui::Hsla::transparent_black()
                                    })
                                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(this) = this_weak_click.upgrade() {
                                            this.update(cx, |this, cx| {
                                                this.update_and_save(&ann_id, |ann| {
                                                    ann.color = color;
                                                    ann.updated_at = chrono::Utc::now().timestamp();
                                                });
                                                this.annotation_state.last_highlight_color = color;
                                                this.annotation_state.context_menu = None;
                                                this.annotation_version += 1;
                                                cx.notify();
                                            });
                                        }
                                    })
                            })
                            .collect::<Vec<_>>(),
                    )
            }));

            // 2. 高亮 / 下划线切换（如果是 text 类型才显示）
            if is_text {
                menu = menu.separator();
                let this_weak_t = this_weak_type.clone();
                let ann_id_t = ann_id_type.clone();
                let current_k = current_kind.clone();
                menu = menu.item(PopupMenuItem::element(move |_window, cx| {
                    let this_weak = this_weak_t.clone();
                    let ann_id = ann_id_t.clone();
                    let current_kind = current_k.clone();

                    let render_btn = {
                        let this_weak = this_weak.clone();
                        let ann_id = ann_id.clone();
                        let current_kind = current_kind.clone();
                        move |kind, cx| {
                            let is_active = current_kind == kind;
                            let lang = this_weak
                                .upgrade()
                                .map(|this| this.read(cx).language)
                                .unwrap_or(i18n::Language::ZhCn);
                            let label = match kind {
                                crate::AnnotationKind::Highlight => {
                                    i18n::t(I18nKey::Highlight, lang)
                                }
                                crate::AnnotationKind::Underline => {
                                    i18n::t(I18nKey::Underline, lang)
                                }
                                _ => "",
                            };
                            let theme = cx.theme();
                            let active_bg = theme.primary.opacity(0.15);
                            let accent_bg = theme.tokens.accent;
                            let accent_fg = theme.tokens.accent_foreground;

                            if is_active {
                                h_flex()
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(active_bg)
                                    .text_xs()
                                    .justify_center()
                                    .items_center()
                                    .child(label)
                                    .into_any_element()
                            } else {
                                let this_weak_click = this_weak.clone();
                                let this_weak_hover = this_weak.clone();
                                let ann_id_click = ann_id.clone();
                                let kind_click = kind.clone();
                                let kind_debug = format!("{:?}", kind);

                                h_flex()
                                    .id(SharedString::from(format!(
                                        "ctx_type_btn_{}_{}",
                                        ann_id_click, kind_debug
                                    )))
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .text_xs()
                                    .cursor_pointer()
                                    .justify_center()
                                    .items_center()
                                    .hover(move |s| s.bg(accent_bg).text_color(accent_fg))
                                    .on_hover(move |_, _, cx| {
                                        if let Some(this) = this_weak_hover.upgrade() {
                                            let _ = this.update(cx, |_, cx| cx.notify());
                                        }
                                    })
                                    .child(label)
                                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(this) = this_weak_click.upgrade() {
                                            this.update(cx, |this, cx| {
                                                this.update_and_save(&ann_id_click, |ann| {
                                                    ann.kind = kind_click.clone();
                                                    ann.updated_at = chrono::Utc::now().timestamp();
                                                });
                                                this.annotation_state.context_menu = None;
                                                this.annotation_version += 1;
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .into_any_element()
                            }
                        }
                    };

                    h_flex()
                        .ml(gpui::px(-16.0))
                        .w_full()
                        .gap_2()
                        .py_1()
                        .child(render_btn(crate::AnnotationKind::Highlight, cx))
                        .child(render_btn(crate::AnnotationKind::Underline, cx))
                }));
            }

            // 3. 矩形截图特有选项
            if let Some((rx, ry, rw, rh)) = rect_coords {
                menu = menu.separator();
                let this_weak_c = this_weak_copy.clone();
                menu = menu.item(
                    PopupMenuItem::new("复制为图片")
                        .icon(Icon::new(PdfIconName::ClipboardCopy))
                        .on_click(move |_, _window, cx| {
                            if let Some(this) = this_weak_c.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.annotation_state.context_menu = None;
                                    this.copy_rect_image(ann_page, rx, ry, rw, rh);
                                    cx.notify();
                                });
                            }
                        }),
                );

                let this_weak_s = this_weak_save.clone();
                menu = menu.item(
                    PopupMenuItem::new("另存为图片")
                        .icon(Icon::new(PdfIconName::FileText))
                        .on_click(move |_, _window, cx| {
                            if let Some(this) = this_weak_s.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.annotation_state.context_menu = None;
                                    this.save_rect_image(ann_page, rx, ry, rw, rh, cx);
                                    cx.notify();
                                });
                            }
                        }),
                );

                let this_weak_p = this_weak_pip.clone();
                menu = menu.item(
                    PopupMenuItem::new(i18n::t(I18nKey::CreatePip, lang))
                        .icon(Icon::new(PdfIconName::Pin))
                        .on_click(move |_, window, cx| {
                            if let Some(this) = this_weak_p.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.annotation_state.context_menu = None;
                                    this.create_pip_from_rect(ann_page, rx, ry, rw, rh, window);
                                    cx.notify();
                                });
                            }
                        }),
                );
            }

            // 4. 查看/添加笔记项
            menu = menu.separator();
            let this_weak_n = this_weak_note.clone();
            let note_label = if has_note {
                i18n::t(I18nKey::ViewNote, lang)
            } else {
                i18n::t(I18nKey::AddNote, lang)
            };
            let ann_id_n = ann_id_note.clone();
            menu = menu.item(
                PopupMenuItem::new(note_label)
                    .icon(Icon::new(PdfIconName::FileText))
                    .on_click(move |_, window, cx| {
                        if let Some(this) = this_weak_n.upgrade() {
                            this.update(cx, |this, cx| {
                                let id = ann_id_n.clone();
                                if let Some(ann) = this.find_annotation(&id) {
                                    let note_text = ann.note.clone().unwrap_or_default();
                                    let input_state = cx.new(|cx| {
                                        InputState::new(window, cx).default_value(note_text)
                                    });
                                    input_state.update(cx, |state, cx| state.focus(window, cx));
                                    this.note_input_state = Some(input_state);
                                    this.annotation_state.note_editor =
                                        Some(crate::NoteEditorState {
                                            annotation_id: id,
                                            position: this
                                                .annotation_state
                                                .context_menu
                                                .as_ref()
                                                .map(|c| c.position)
                                                .unwrap_or(gpui::point(px(0.0), px(0.0))),
                                        });
                                }
                                this.overlay_button_clicked = true;
                                this.annotation_state.context_menu = None;
                                cx.notify();
                            });
                        }
                    }),
            );

            // 5. 删除项
            menu = menu.separator();
            let this_weak_d = this_weak_delete.clone();
            let ann_id_d = ann_id_delete.clone();
            menu = menu.item(
                PopupMenuItem::new(i18n::t(I18nKey::Delete, lang))
                    .icon(Icon::new(PdfIconName::Close))
                    .on_click(move |_, _window, cx| {
                        if let Some(this) = this_weak_d.upgrade() {
                            this.update(cx, |this, cx| {
                                let id = ann_id_d.clone();
                                this.annotation_state.context_menu = None;
                                this.annotation_state.selected_id = None;
                                if let Some(ann) = this.find_annotation_mut(&id) {
                                    ann.is_deleted = true;
                                }
                                if let Some(delegate) = &this.delegate {
                                    delegate.delete_annotation(&id);
                                }
                                this.annotation_version += 1;
                                cx.notify();
                            });
                        }
                    }),
            );

            menu
        });

        let element = anchored().position(adjusted_pos).child(menu);
        Some(element.into_any_element())
    }

    pub(crate) fn render_note_editor(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let _note_state = self.annotation_state.note_editor.as_ref()?;
        let input_state = self.note_input_state.as_ref()?;

        let theme = cx.theme();
        let bg = theme.background;
        let border = theme.border;

        let pos_x = f32::from(_note_state.position.x);
        let toolbar_height_px =
            f32::from(gpui::rems(TOOLBAR_HEIGHT_REMS).to_pixels(window.rem_size()));
        let tab_bar_h = self.tab_bar_offset_px;
        let pos_y = f32::from(_note_state.position.y) - tab_bar_h - toolbar_height_px;
        let viewport_w = f32::from(window.viewport_size().width);
        let viewport_h = f32::from(window.viewport_size().height) - tab_bar_h - toolbar_height_px;

        const POPUP_W: f32 = 300.0;

        let px_pos_x = pos_x.clamp(0.0, (viewport_w - POPUP_W).max(0.0));
        let py_pos_y = if pos_y + 200.0 > viewport_h {
            (pos_y - 200.0).max(0.0)
        } else {
            pos_y
        };

        let outer_id = "note-editor-overlay";
        Some(
            div()
                .absolute()
                .left(px(px_pos_x))
                .top(px(py_pos_y))
                .w(px(POPUP_W))
                .bg(bg)
                .border_1()
                .border_color(border)
                .shadow_lg()
                .rounded_md()
                .p_3()
                .cursor_default()
                .id(outer_id)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, _| {
                        this.overlay_button_clicked = true;
                    }),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(Input::new(input_state).w_full())
                        .child(
                            h_flex()
                                .w_full()
                                .justify_end()
                                .gap_2()
                                .child(
                                    div()
                                        .id("note_editor_check")
                                        .cursor_pointer()
                                        .rounded_sm()
                                        .hover(move |s| s.bg(theme.muted))
                                        .on_hover(cx.listener(move |_this, _, _, cx| {
                                            cx.notify();
                                        }))
                                        .child(Icon::new(PdfIconName::Check).size(px(16.0)))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.save_note_and_close(cx);
                                            }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("note_editor_close")
                                        .cursor_pointer()
                                        .rounded_sm()
                                        .hover(move |s| s.bg(theme.muted))
                                        .on_hover(cx.listener(move |_this, _, _, cx| {
                                            cx.notify();
                                        }))
                                        .child(Icon::new(PdfIconName::Close).size(px(16.0)))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.close_note_editor(cx);
                                            }),
                                        ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    fn save_note_and_close(&mut self, cx: &mut Context<Self>) {
        let text = self
            .note_input_state
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();

        if let Some(editor) = &self.annotation_state.note_editor {
            let id = editor.annotation_id.clone();
            self.update_and_save(&id, |ann| {
                ann.note = Some(text);
                ann.updated_at = chrono::Utc::now().timestamp();
            });
        }

        self.note_input_state = None;
        self.note_input_sub = None;
        self.annotation_state.note_editor = None;
        cx.notify();
    }

    pub(crate) fn close_note_editor(&mut self, cx: &mut Context<Self>) {
        self.note_input_state = None;
        self.note_input_sub = None;
        self.annotation_state.note_editor = None;
        cx.notify();
    }

    pub(crate) fn save_sidebar_note(&mut self, cx: &mut Context<Self>) {
        let id = match self.editing_note_sidebar_id.clone() {
            Some(id) => id,
            None => return,
        };
        let text = self
            .editing_note_sidebar_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        self.update_and_save(&id, |ann| {
            ann.note = Some(text);
            ann.updated_at = chrono::Utc::now().timestamp();
        });
        self.editing_note_sidebar_id = None;
        self.editing_note_sidebar_input = None;
        self.editing_note_sidebar_sub = None;
        cx.notify();
    }

    pub(crate) fn cancel_sidebar_note(&mut self, cx: &mut Context<Self>) {
        self.editing_note_sidebar_id = None;
        self.editing_note_sidebar_input = None;
        self.editing_note_sidebar_sub = None;
        cx.notify();
    }

    pub(crate) fn render_annotation_toolbar(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (pos_x, pos_y) = self.compute_toolbar_screen_pos(window)?;

        let theme = cx.theme();
        let bg_color = theme.background;
        let border_color = theme.border;

        Some(
            div()
                .absolute()
                .left(pos_x)
                .top(pos_y)
                .bg(bg_color)
                .border_1()
                .border_color(border_color)
                .shadow_lg()
                .rounded_md()
                .p_2()
                .cursor_default()
                .child(
                    h_flex()
                        .gap_1()
                        .px_1()
                        .py_1()
                        .child(self.render_toolbar_color_dot(AnnotationColor::Yellow, cx))
                        .child(self.render_toolbar_color_dot(AnnotationColor::Red, cx))
                        .child(self.render_toolbar_color_dot(AnnotationColor::Green, cx))
                        .child(self.render_toolbar_color_dot(AnnotationColor::Blue, cx))
                        .child(self.render_toolbar_color_dot(AnnotationColor::Purple, cx))
                        .child(self.render_toolbar_color_dot(AnnotationColor::Magenta, cx))
                        .child(self.render_toolbar_color_dot(AnnotationColor::Orange, cx))
                        .child(self.render_toolbar_color_dot(AnnotationColor::Gray, cx)),
                )
                .child(div().h_px().bg(border_color).my_1())
                .child(
                    h_flex()
                        .w_full()
                        .px_1()
                        .py_1()
                        .gap_2()
                        .child(
                            div().flex_1().child(
                                self.render_type_button(ToolbarAnnotationKind::Highlight, cx),
                            ),
                        )
                        .child(
                            div().flex_1().child(
                                self.render_type_button(ToolbarAnnotationKind::Underline, cx),
                            ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// 计算工具栏在屏幕（窗口）坐标系中的位置，包含碰撞检测
    fn compute_toolbar_screen_pos(&mut self, window: &Window) -> Option<(Pixels, Pixels)> {
        let state = self.annotation_state.toolbar.as_ref()?;

        // 跨页时取首页作为工具栏定位参考
        let page_index = state.start_page as usize;

        let rem_size = window.rem_size();
        let rem_size_px = f32::from(rem_size);
        let display_width_px = PAGE_BASE_WIDTH_REMS * self.zoom_level * rem_size_px;

        // 1. 选中文本在页面内的包围盒（取首页）
        // 跨页时 end_char 在 end_page 上，首页应截断到页尾
        let (min_x, min_y, max_x, max_y) = self
            .page_text_data
            .get(state.start_page as usize)
            .and_then(|d| d.as_ref())
            .and_then(|data| {
                let end_on_page = if state.start_page == state.end_page {
                    state.end_char
                } else {
                    data.chars.len().saturating_sub(1)
                };
                if state.start_char > end_on_page {
                    return None;
                }
                let blocks = data.merge_char_blocks(state.start_char, end_on_page);
                if blocks.is_empty() {
                    return None;
                }
                let (mut mnx, mut mny, mut mxx, mut mxy) = blocks[0];
                for &(bx, by, bx2, by2) in &blocks {
                    mnx = mnx.min(bx);
                    mny = mny.min(by);
                    mxx = mxx.max(bx2);
                    mxy = mxy.max(by2);
                }
                Some((mnx, mny, mxx, mxy))
            })?;

        // 2. 该页在视口中的屏幕 Y 位置
        let toolbar_height_px = f32::from(gpui::rems(TOOLBAR_HEIGHT_REMS).to_pixels(rem_size));
        let tab_bar_h = self.tab_bar_offset_px;
        let scroll_top = self.list_state.logical_scroll_top();

        if page_index < scroll_top.item_ix {
            return None;
        }

        let mut page_screen_top = 0.0_f32;
        for ix in scroll_top.item_ix..page_index {
            page_screen_top +=
                helpers::page_height(&self.page_sizes, ix, self.zoom_level, rem_size_px);
        }
        page_screen_top -= f32::from(scroll_top.offset_in_item);

        // 3. 页面水平居中偏移
        let mut available_w = f32::from(window.viewport_size().width);
        let mut offset_x = 0.0;
        if self.is_left_sidebar_open {
            let w = f32::from(self.left_sidebar_width);
            available_w -= w;
            offset_x = w;
        }
        if self.is_right_sidebar_open {
            available_w -= f32::from(self.right_sidebar_width);
        }
        let page_screen_left = offset_x + (available_w - display_width_px) / 2.0 + self.offset_x;

        // 4. 选中文本的中心屏幕坐标
        let center_screen_x = page_screen_left + (min_x + max_x) / 2.0;
        let text_bottom_screen_y = page_screen_top + max_y;
        let text_top_screen_y = page_screen_top + min_y;

        // 5. 碰撞检测（视口边界）
        let viewport_w = f32::from(window.viewport_size().width);
        let viewport_h = f32::from(window.viewport_size().height) - tab_bar_h - toolbar_height_px;

        const TOOLBAR_W: f32 = 200.0;
        const TOOLBAR_H: f32 = 80.0;

        let tool_x =
            (center_screen_x - TOOLBAR_W / 2.0).clamp(0.0, (viewport_w - TOOLBAR_W).max(0.0));
        let mut tool_y = text_bottom_screen_y + 5.0;

        if tool_y + TOOLBAR_H > viewport_h {
            tool_y = (text_top_screen_y - TOOLBAR_H - 12.0).max(0.0);
        }

        Some((px(tool_x), px(tool_y)))
    }

    fn render_toolbar_color_dot(
        &self,
        color: AnnotationColor,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let hex = color.to_hex();
        let color_val = u32::from_str_radix(&hex[1..], 16).unwrap_or(0x000000);
        let r = ((color_val >> 16) & 0xFF) as f32 / 255.0;
        let g = ((color_val >> 8) & 0xFF) as f32 / 255.0;
        let b = (color_val & 0xFF) as f32 / 255.0;
        let hsla = gpui::Hsla::from(gpui::Rgba { r, g, b, a: 1.0 });

        div()
            .size_4()
            .rounded_full()
            .bg(hsla)
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if let Some(ref toolbar) = this.annotation_state.toolbar {
                        let kind = match this.annotation_state.toolbar_kind {
                            ToolbarAnnotationKind::Highlight => crate::AnnotationKind::Highlight,
                            ToolbarAnnotationKind::Underline => crate::AnnotationKind::Underline,
                        };
                        this.annotation_state.last_highlight_color = color;
                        this.create_annotation_from_selection(
                            toolbar.start_page,
                            toolbar.start_char,
                            toolbar.end_page,
                            toolbar.end_char,
                            kind,
                            color,
                            cx,
                        );
                    }
                    this.close_annotation_toolbar(cx);
                }),
            )
    }

    fn render_type_button(
        &self,
        kind: ToolbarAnnotationKind,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let is_active = self.annotation_state.toolbar_kind == kind;
        let label = match kind {
            ToolbarAnnotationKind::Highlight => i18n::t(I18nKey::Highlight, self.language),
            ToolbarAnnotationKind::Underline => i18n::t(I18nKey::Underline, self.language),
        };
        let theme = cx.theme();
        let active_bg = theme.primary.opacity(0.15);
        let accent_bg = theme.tokens.accent;
        let accent_fg = theme.tokens.accent_foreground;
        let kind_debug = format!("{:?}", kind);

        h_flex()
            .id(SharedString::from(format!(
                "toolbar_type_btn_{}",
                kind_debug
            )))
            .w_full()
            .px_2()
            .py_1()
            .rounded_sm()
            .text_xs()
            .cursor_pointer()
            .justify_center()
            .items_center()
            .when(is_active, |this| this.bg(active_bg))
            .hover(move |s| {
                if is_active {
                    s.bg(active_bg)
                } else {
                    s.bg(accent_bg).text_color(accent_fg)
                }
            })
            .on_hover(cx.listener(move |_this, _, _, cx| {
                cx.notify();
            }))
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.annotation_state.toolbar_kind = kind;
                    this.overlay_button_clicked = true;
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    pub(crate) fn close_annotation_toolbar(&mut self, cx: &mut Context<Self>) {
        self.annotation_state.toolbar = None;
        self.selection_start = None;
        self.selection_end = None;
        self.selected_text = None;
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create_annotation_from_selection(
        &mut self,
        start_page: u16,
        start_char: usize,
        end_page: u16,
        end_char: usize,
        kind: crate::AnnotationKind,
        color: AnnotationColor,
        cx: &mut Context<Self>,
    ) {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let annotation = crate::Annotation {
            id: id.clone(),
            document_id: self.document_id.clone(),
            page: start_page,
            kind,
            color,
            range: Some(crate::TextRange {
                start_page,
                start_char,
                end_page: if end_page != start_page {
                    Some(end_page)
                } else {
                    None
                },
                end_char,
            }),
            note: None,
            created_at: now,
            updated_at: now,
            version: 1,
            is_deleted: false,
            is_dirty: true,
        };

        self.annotation_state
            .annotations
            .entry(start_page)
            .or_default()
            .push(annotation.clone());

        if let Some(delegate) = &self.delegate {
            delegate.save_annotation(&annotation);
        }
        self.annotation_version += 1;
        cx.notify();
    }

    /// 将矩形标注区域从原始页面图中裁剪并复制到剪贴板。
    fn copy_rect_image(&mut self, page: u16, x: f32, y: f32, w: f32, h: f32) {
        let img = match self.raw_page_images.get(page as usize) {
            Some(Some(raw)) => &**raw,
            _ => return,
        };
        let (iw, ih) = (img.width() as f32, img.height() as f32);
        let cx = (x * iw) as u32;
        let cy = (y * ih) as u32;
        let cw = (w * iw) as u32;
        let ch = (h * ih) as u32;
        let cropped = image::imageops::crop_imm(img, cx, cy, cw, ch).to_image();
        helpers::copy_rgba_to_clipboard(&cropped);
    }

    /// 将矩形标注区域裁剪并另存为 PNG 文件。
    fn save_rect_image(
        &mut self,
        page: u16,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        cx: &mut Context<Self>,
    ) {
        let img = match self.raw_page_images.get(page as usize) {
            Some(Some(raw)) => &**raw,
            _ => return,
        };
        let (iw, ih) = (img.width() as f32, img.height() as f32);
        let crop_x = (x * iw) as u32;
        let crop_y = (y * ih) as u32;
        let crop_w = (w * iw) as u32;
        let crop_h = (h * ih) as u32;
        let cropped = image::imageops::crop_imm(img, crop_x, crop_y, crop_w, crop_h).to_image();
        let name = self.document_title.clone();

        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择保存位置".into()),
        });

        cx.background_executor()
            .spawn(async move {
                if let Ok(Ok(Some(paths))) = receiver.await
                    && let Some(dir) = paths.first()
                {
                    let path = dir.join(format!("{}.png", name));
                    let _ = cropped.save(&path);
                }
            })
            .detach();
    }
}
