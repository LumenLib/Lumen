use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result};

#[cfg(feature = "gpui")]
use gpui::SharedString;
#[cfg(feature = "gpui")]
use gpui_component::select::SelectItem;

mod de;
mod en_us;
mod es;
mod fr;
mod ja;
mod ko;
pub mod literature_type;
mod ru;
mod zh_cn;
mod zh_tw;

pub use literature_type::LiteratureTypeExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    ZhCn,
    ZhTw,
    En,
    Ja,
    Ko,
    Ru,
    Fr,
    De,
    Es,
}

impl Language {
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::ZhCn => "简体中文",
            Self::ZhTw => "繁體中文",
            Self::En => "English",
            Self::Ja => "日本語",
            Self::Ko => "한국어",
            Self::Ru => "Русский",
            Self::Fr => "Français",
            Self::De => "Deutsch",
            Self::Es => "Español",
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::ZhTw => "zh-TW",
            Self::En => "en-US",
            Self::Ja => "ja-JP",
            Self::Ko => "ko-KR",
            Self::Ru => "ru-RU",
            Self::Fr => "fr-FR",
            Self::De => "de-DE",
            Self::Es => "es-ES",
        }
    }
}

impl std::str::FromStr for Language {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "zh-TW" | "zh-HK" => Ok(Self::ZhTw),
            "en-US" | "en" => Ok(Self::En),
            "ja-JP" | "ja" => Ok(Self::Ja),
            "ko-KR" | "ko" => Ok(Self::Ko),
            "ru-RU" | "ru" => Ok(Self::Ru),
            "fr-FR" | "fr" => Ok(Self::Fr),
            "de-DE" | "de" => Ok(Self::De),
            "es-ES" | "es" => Ok(Self::Es),
            "zh-CN" | "zh" => Ok(Self::ZhCn),
            _ => Err(()),
        }
    }
}

#[cfg(feature = "gpui")]
impl SelectItem for Language {
    type Value = Language;

    fn title(&self) -> SharedString {
        self.name().into()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

impl Display for Language {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.name())
    }
}

pub trait Translatable {
    fn translate(&self, lang: Language) -> &'static str;
}

#[derive(Debug, Clone, Copy)]
pub enum I18nKey {
    // Tab
    Library,
    Subscription,

    // Folder Panel
    AllLiterature,
    Uncategorized,
    Trash,
    Tags,
    AllSubscription,
    Unread,
    StatusReading,
    StatusRead,
    FolderNamePlaceholder,
    TagNamePlaceholder,
    SearchOrCreateTags,
    CreateTag,
    AddTag,
    Version,
    EmptyFolder,
    NoMatchFound,

    // Toolbar
    SearchPlaceholder,
    SearchBoxPlaceholder,
    AddLiterature,
    ManualAdd,
    BibTeXImport,
    DoiImport,
    ArXivImport,
    DblpSearch,
    FetchMetadata,
    CompareLiterature,
    ExportLiterature,
    FindDuplicates,
    DuplicateGroups,
    SyncConflicts,
    DuplicateDetection,
    NoDuplicatesFound,

    // Metadata Selector
    SelectMetadataCandidate,

    // Context Menu
    NewFolder,
    EmptyTrash,
    Rename,
    Delete,
    NewSubscription,
    Refresh,
    OpenInBrowser,
    CopyDoi,
    CopyBibtex,
    MarkAsRead,
    MarkAsUnread,
    UpdateSubscription,
    EditSubscription,
    Unsubscribe,
    AddSubscription,
    NewSubFolder,
    Edit,
    Quit,
    PermanentDelete,
    CopyCitation,
    FetchFrom,
    BatchFetchMetadata,
    AddTo,
    RestoreTo,
    RemoveFromFolder,
    RevealInFinder,
    RevealInExplorer,
    ReplaceFile,
    DeleteFile,
    SelectNewFile,
    Confirm,
    LoadingMetadata,
    FetchFailed,
    Retry,
    Close,
    ConfirmFetch,
    FetchFromSource,
    FetchPlaceholderDoi,
    FetchPlaceholderArxiv,
    FetchPlaceholderBibtex,
    FetchPlaceholderDblp,
    FetchPlaceholderOpenAlex,
    NoContentOrInvalidFormat,
    ImportFailed,

    // Literature Editor
    LiteratureEditor,
    AuthorPlaceholder,
    JournalPlaceholder,
    Month,
    Day,
    Publisher,
    SaveLiterature,

    // Literature Compare
    Field,
    LocalData,
    RemoteData,
    CompareAndMerge,

    // Subscription Editor
    SubscriptionEditor,
    FeedName,
    FeedUrl,
    UpdateInterval,
    Hours,
    SubscriptionNamePlaceholder,
    SubscriptionUrlPlaceholder,
    UpdateIntervalPlaceholder,
    Add,

    // Subscription Detail
    SelectedSubscriptionCount,
    BatchAddToLibrary,
    AddedToLibrary,
    AddToLibrary,
    FetchTime,
    UpdatedAt,
    NoSubscriptionSelected,
    NoAbstract,

    // Citation Popup
    CopyCitationTitle,
    Style,
    Preview,
    NoLiteratureSelectedForCitation,
    CitationError,

    // Literature Types
    Type,
    TypeArticle,
    TypeBook,
    TypeConference,
    TypeThesis,
    TypePreprint,
    TypeTechnicalReport,
    TypeWebpage,
    TypeOther,

    // Folder Types
    TypeAll,
    TypeCustom,
    TypeUncategorized,
    TypeTrash,

    // Literature Detail
    Folders,
    Title,
    Authors,
    Journal,
    Year,
    Volume,
    Issue,
    Pages,
    Url,
    Doi,
    ArXiv,
    Abstract,
    Keywords,
    Notes,
    Attachments,
    NoLiteratureSelected,
    SelectedCount,
    MainFile,
    Attachment,
    SetAsMainFile,
    SetAsAttachment,
    Expand,
    Collapse,
    Publication,
    RelatedLiterature,
    AddCitation,
    References,
    CitedBy,

    // Settings
    Settings,
    Language,
    Appearance,
    UiScale,
    LogLevel,
    ThemeStyle,
    Theme,
    Dark,
    Light,
    System,
    General,
    Sync,
    About,
    Cancel,
    Save,
    LibrarySettings,
    AttachmentDir,
    AttachmentDirDesc,
    DatabaseDir,
    DatabaseDirDesc,
    LogDir,
    LogDirDesc,
    ThemeDir,
    ThemeDirDesc,
    FilenameTemplate,
    FilenameTemplateDesc,
    BatchRename,
    CleanupOrphanedFiles,
    GeneralOptions,
    ThemeDesc,
    CloudSyncDesc,
    AboutDesc,
    Copyright,

    // Advanced Filter
    AdvancedFilter,
    Filter,
    Reset,
    YearStart,
    YearEnd,

    // Sort
    SortBy,
    SortByTitle,
    SortByAuthor,
    SortByYear,
    SortByJournal,
    SortAscending,
    SortDescending,

    // Sync
    SyncNow,
    SyncMetadata,
    SyncAttachments,
    TestConnection,
    SyncSuccess,
    SyncFailed,
    WebDavSettings,
    DatabaseSettings,
    EndpointUrl,
    Username,
    Password,
    RemotePath,
    Host,
    Port,
    DatabaseName,
    EnableSSL,
    ConnectionSuccess,
    ConnectionFailed,
    OnDemandDownload,
    OnDemandDownloadDesc,
    SyncMetadataTab,
    SyncAttachmentTab,
    GoogleDriveSettings,
    GoogleDriveDesc,
    ClientId,
    ClientSecret,
    Authorize,
    DataManagement,
    ClearLocalDb,
    ClearLocalFiles,
    ClearCloudDb,
    ClearCloudFiles,

    // PDF Viewer
    PdfViewerSettings,
    PdfViewerSettingsDesc,
    UseCustomPdfViewer,
    PdfViewerPath,
    PdfViewerPathMacos,
    PdfViewerPathWindows,
    PdfViewerPathPlaceholderMacos,
    PdfViewerPathPlaceholderWindows,
    Browse,

    // Metadata Services
    MetadataServices,
    EasyScholarKey,
    EasyScholarDesc,
    EasyScholarPlaceholder,

    // Service Errors
    LiteratureNotFound,
    SubscriptionNotFound,
    FeedItemNotFound,
    LiteratureAddedNoRecord,
    LiteratureNotFoundGeneric,
    AttachmentNotFoundById,
    FileNotFoundPath,
    AttachmentNotFound,

    // Error/Notification
    FileNotFoundTitle,
    FileNotFoundMsg,
    DataConsistentTitle,
    DataConsistentMsg,
    LiteratureMergedTitle,
    LiteratureMergedMsg,

    // Fetch Error Tips
    FetchFailedArxiv,
    FetchFailedDblp,
    FetchFailedCrossref,
    FetchFailedOpenAlex,

    // Batch Update
    BatchUpdatingMetadata,

    // Settings - Translation
    TranslationSettings,
    TranslationSettingsDesc,
    TranslationEngine,
    TranslationSettingsTab,
    NoApiKeyRequired,
    NiuTransApiKey,
    InternalReaderDesc,
    SelectDirectory,
    SelectMacosPdfReader,
    SelectWindowsPdfReader,

    // PDF View - Notes
    EditNotesMarkdown,

    // Feed
    UnknownFeedSource,

    // Bookmark
    UnnamedBookmark,

    // Pdf Viewer
    ToggleLeftSidebar,
    SearchDocument,
    RectangleSelect,
    ZoomOut,
    ZoomIn,
    FitWidth,
    ToggleRightSidebar,
    NotePlaceholder,
    ViewNote,
    AddNote,
    Highlight,
    Underline,
    LoadingOutline,
    NoOutline,
    RectangleAnnotation,
    PageRange,
    SinglePage,
    SelectTextToTranslate,
    OriginalSection,
    TranslationSection,
    Translating,
    TranslationPending,
    NoNotes,
    TranslateEngine,
    NiuTrans,
    Copy,
    PdfEngineError,
    CloseWindow,
    TranslationNotImplemented,

    // PDF Search
    SearchInPdf,
    NextMatch,
    PrevMatch,
    SearchInputPlaceholder,
    NoSearchResults,
}

impl Translatable for I18nKey {
    fn translate(&self, lang: Language) -> &'static str {
        match lang {
            Language::ZhCn => zh_cn::translate(*self),
            Language::ZhTw => zh_tw::translate(*self),
            Language::En => en_us::translate(*self),
            Language::Ja => ja::translate(*self),
            Language::Ko => ko::translate(*self),
            Language::Ru => ru::translate(*self),
            Language::Fr => fr::translate(*self),
            Language::De => de::translate(*self),
            Language::Es => es::translate(*self),
        }
    }
}

#[must_use]
pub fn t(key: I18nKey, lang: Language) -> &'static str {
    key.translate(lang)
}

#[must_use]
pub fn tf(key: I18nKey, lang: Language, args: &[&str]) -> String {
    let mut s = t(key, lang).to_string();
    for arg in args {
        s = s.replacen("{}", arg, 1);
    }
    s
}
