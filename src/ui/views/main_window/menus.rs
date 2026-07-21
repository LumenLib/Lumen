//! 原生菜单构建
//!
//! 根据当前视图模式（文献库 / 订阅）构建 macOS 原生菜单栏。
//! 菜单完全由 `cx.set_menus` 决定，GPUI 不会自动补系统菜单项，
//! 因此 App（关于/设置/隐藏/退出）与上下文菜单（文献库/订阅）需自行定义。

use super::{ShowAbout, ShowSettings};
use crate::actions::*;
use crate::services::AppViewMode;
use gpui::{Menu, MenuItem, SystemMenuType};
use i18n::{I18nKey, Language, t};

const APP_NAME: &str = "Lumen";

/// App 菜单（第一个菜单即 macOS 左上角应用菜单）
fn app_menu(lang: Language) -> Menu {
    Menu {
        name: APP_NAME.into(),
        disabled: false,
        items: vec![
            MenuItem::action(t(I18nKey::About, lang), ShowAbout),
            MenuItem::separator(),
            MenuItem::action(t(I18nKey::Settings, lang), ShowSettings),
            MenuItem::separator(),
            // Services 子菜单由系统填充（macOS 原生），标题随 app 语言本地化
            MenuItem::os_submenu(t(I18nKey::Services, lang), SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(t(I18nKey::Hide, lang), HideApp),
            MenuItem::action(t(I18nKey::HideOthers, lang), HideOtherApps),
            MenuItem::action(t(I18nKey::ShowAll, lang), ShowAllApps),
            MenuItem::separator(),
            MenuItem::action(t(I18nKey::Quit, lang), Quit),
        ],
    }
}

/// 文献库菜单：添加（各源子菜单）+ 重复文献搜索
fn library_menu(lang: Language) -> Menu {
    let add_submenu = Menu {
        name: t(I18nKey::Add, lang).into(),
        disabled: false,
        items: vec![
            MenuItem::action(t(I18nKey::ManualAdd, lang), AddSourceManual),
            MenuItem::separator(),
            MenuItem::action("BibTeX", AddSourceBibtex),
            MenuItem::action("DOI", AddSourceDoi),
            MenuItem::action("ArXiv", AddSourceArxiv),
            MenuItem::action("DBLP", AddSourceDblp),
            MenuItem::action("OpenAlex", AddSourceOpenalex),
        ],
    };

    Menu {
        name: t(I18nKey::Library, lang).into(),
        disabled: false,
        items: vec![
            MenuItem::submenu(add_submenu),
            MenuItem::action(t(I18nKey::DuplicateSearch, lang), DuplicateSearch),
        ],
    }
}

/// 订阅菜单：添加订阅
fn subscription_menu(lang: Language) -> Menu {
    Menu {
        name: t(I18nKey::Subscription, lang).into(),
        disabled: false,
        items: vec![MenuItem::action(
            t(I18nKey::AddSubscription, lang),
            AddSubscription,
        )],
    }
}

/// 根据当前视图模式构建完整菜单栏
pub fn build_app_menus(view_mode: AppViewMode, lang: Language) -> Vec<Menu> {
    let mut menus = vec![app_menu(lang)];

    // 上下文菜单：随视图模式切换
    match view_mode {
        AppViewMode::Library => menus.push(library_menu(lang)),
        AppViewMode::Subscription => menus.push(subscription_menu(lang)),
    }

    menus
}
