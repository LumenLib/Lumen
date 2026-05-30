use crate::config_store::ConfigStore;
use gpui::{App, Global, SharedString};
use gpui_component::notification::NotificationType;
use log::error as log_error;

#[derive(Clone)]
pub struct NotificationItem {
    pub ty: NotificationType,
    pub message: SharedString,
}

pub struct NotificationBus {
    pending: Vec<NotificationItem>,
}

impl Global for NotificationBus {}

impl NotificationBus {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    pub fn push(&mut self, item: NotificationItem) {
        self.pending.push(item);
    }

    pub fn drain(&mut self) -> Vec<NotificationItem> {
        std::mem::take(&mut self.pending)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotificationLevel {
    Error = 2,
    Warning = 1,
    All = 0,
}

impl NotificationLevel {
    pub fn should_show(&self, ty: NotificationType) -> bool {
        match (self, ty) {
            (NotificationLevel::Error, NotificationType::Error) => true,
            (NotificationLevel::Warning, NotificationType::Error | NotificationType::Warning) => {
                true
            }
            (NotificationLevel::All, _) => true,
            _ => false,
        }
    }
}

pub fn level_from_config(s: &str) -> NotificationLevel {
    match s {
        "error" => NotificationLevel::Error,
        "warn" => NotificationLevel::Warning,
        _ => NotificationLevel::All,
    }
}

pub fn show_notification(ty: NotificationType, message: impl Into<SharedString>, cx: &mut App) {
    let level = {
        let store = cx.global::<ConfigStore>();
        level_from_config(&store.notification_level)
    };
    let msg: SharedString = message.into();

    let level_name = match ty {
        NotificationType::Info => "info",
        NotificationType::Success => "success",
        NotificationType::Warning => "warn",
        NotificationType::Error => "error",
    };

    if !level.should_show(ty) {
        log_error!("[notification suppressed: {level_name}] {msg}");
        return;
    }

    log_error!("[notification: {level_name}] {msg}");

    cx.global_mut::<NotificationBus>().push(NotificationItem {
        ty,
        message: msg,
    });
}
