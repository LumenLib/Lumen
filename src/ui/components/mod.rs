pub mod citation_popup;
pub mod detail_helper;

use gpui::{Div, ParentElement, Styled, div};
use gpui_component::{
    Theme,
    input::Input,
    select::{Select, SelectDelegate},
};
pub mod duplicate_list;
pub mod folder_selector;
pub mod form;
pub mod literature_compare;
pub mod literature_editor;
pub mod literature_fetcher;
pub mod metadata_selector;
pub mod modal;
pub mod resize_handle;
pub mod setting;
pub mod subscription_editor;
pub mod tag_selector;
pub mod toast;

pub use citation_popup::CitationPopup;
pub use detail_helper::{
    CollapsibleText, DetailRow, LinkRow, render_copy_button, render_icon_button,
};
pub use duplicate_list::DuplicateList;
pub use folder_selector::FolderSelector;
pub use form::LabeledInput;
pub use literature_compare::{FieldSelection, LiteratureCompare};
pub use literature_editor::LiteratureEditor;
pub use literature_fetcher::{FetchMode, LiteratureFetcher};
pub use metadata_selector::MetadataSelector;
pub use modal::render_modal_overlay;
pub use setting::{SettingsTab, SettingsWindow};
pub use subscription_editor::SubscriptionEditor;
pub use tag_selector::TagSelector;
pub use toast::ToastOverlay;

/// Wrap an Input with muted background, border, and rounded corners.
pub fn muted_input(input: Input, theme: &Theme) -> Div {
    div()
        .bg(theme.muted)
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .child(input.appearance(false))
}

/// Wrap a Select with muted background and rounded corners.
pub fn muted_select<D: SelectDelegate + 'static>(
    select: Select<D>,
    theme: &Theme,
) -> Div {
    div()
        .bg(theme.muted)
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .child(select.appearance(false))
}
