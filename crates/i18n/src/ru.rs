use crate::I18nKey;

pub fn translate(key: I18nKey) -> &'static str {
    match key {
        I18nKey::Library => "Библиотека",
        I18nKey::Subscription => "Подписки",
        I18nKey::AllLiterature => "Вся литература",
        I18nKey::Uncategorized => "Без категории",
        I18nKey::Trash => "Корзина",
        I18nKey::Tags => "Теги",
        I18nKey::AllSubscription => "Все подписки",
        I18nKey::Unread => "Непрочитанные",
        I18nKey::StatusReading => "Читаю",
        I18nKey::StatusRead => "Прочитано",
        I18nKey::FolderNamePlaceholder => "Имя папки",
        I18nKey::TagNamePlaceholder => "Имя тега",
        I18nKey::Version => "Версия",
        I18nKey::EmptyFolder => "Папка пуста",
        I18nKey::NoMatchFound => "Совпадений не найдено",
        I18nKey::SearchPlaceholder => "Поиск...",
        I18nKey::SearchBoxPlaceholder => "Поиск по названию, автору или журналу...",
        I18nKey::AddLiterature => "Добавить литературу",
        I18nKey::ManualAdd => "Добавить вручную",
        I18nKey::BibTeXImport => "Импорт BibTeX",
        I18nKey::DoiImport => "Импорт DOI",
        I18nKey::ArXivImport => "Импорт ArXiv",
        I18nKey::DblpSearch => "Поиск DBLP",
        I18nKey::FetchMetadata => "Получить метаданные",
        I18nKey::CompareLiterature => "Сравнить",
        I18nKey::ExportLiterature => "Экспорт",
        I18nKey::FindDuplicates => "Find Duplicates",
        I18nKey::DuplicateGroups => "Duplicate Groups",
        I18nKey::SyncConflicts => "Version Conflicts",
        I18nKey::DuplicateDetection => "Duplicate Detection",
        I18nKey::NoDuplicatesFound => "No duplicates found",
        I18nKey::NewFolder => "Новая папка",
        I18nKey::EmptyTrash => "Очистить корзину",
        I18nKey::Rename => "Переименовать",
        I18nKey::Delete => "Удалить",
        I18nKey::NewSubscription => "Новая подписка",
        I18nKey::Refresh => "Обновить",
        I18nKey::OpenInBrowser => "Открыть в браузере",
        I18nKey::CopyDoi => "Копировать DOI",
        I18nKey::CopyBibtex => "Копировать BibTeX",
        I18nKey::MarkAsRead => "Отметить как прочитанное",
        I18nKey::MarkAsUnread => "Отметить как непрочитанное",
        I18nKey::UpdateSubscription => "Обновить",
        I18nKey::EditSubscription => "Редактировать подписку",
        I18nKey::Unsubscribe => "Отписаться",
        I18nKey::AddSubscription => "Добавить подписку",
        I18nKey::NewSubFolder => "Создать подпапку",
        I18nKey::Edit => "Редактировать",
        I18nKey::Quit => "Выйти",
        I18nKey::PermanentDelete => "Удалить навсегда",
        I18nKey::CopyCitation => "Копировать цитату",
        I18nKey::FetchFrom => "Получить из...",
        I18nKey::BatchFetchMetadata => "Пакетное обновление метаданных",
        I18nKey::AddTo => "Добавить в...",
        I18nKey::RestoreTo => "Восстановить в...",
        I18nKey::RemoveFromFolder => "Удалить из папки",
        I18nKey::RevealInFinder => "Показать в Finder",
        I18nKey::RevealInExplorer => "Показать в Проводнике",
        I18nKey::ReplaceFile => "Заменить файл",
        I18nKey::DeleteFile => "Удалить файл",
        I18nKey::SelectNewFile => "Выбрать новый файл",
        I18nKey::Confirm => "ОК",
        I18nKey::LoadingMetadata => "Получение метаданных...",
        I18nKey::FetchFailed => "Ошибка получения",
        I18nKey::Retry => "Повторить",
        I18nKey::Close => "Закрыть",
        I18nKey::ConfirmFetch => "Получить",
        I18nKey::FetchFromSource => "Получить из {}",
        I18nKey::FetchPlaceholderDoi => "Введите DOI",
        I18nKey::FetchPlaceholderArxiv => "Введите ArXiv ID",
        I18nKey::FetchPlaceholderBibtex => "Вставьте BibTeX",
        I18nKey::FetchPlaceholderDblp => "Поиск в DBLP",
        I18nKey::FetchPlaceholderOpenAlex => "Поиск в OpenAlex",
        I18nKey::NoContentOrInvalidFormat => "Пусто или неверный формат",
        I18nKey::ImportFailed => "Ошибка импорта",

        I18nKey::LiteratureEditor => "Редактор литературы",
        I18nKey::AuthorPlaceholder => "Авторы (через запятую)",
        I18nKey::JournalPlaceholder => "Журнал / Конференция / Книга",
        I18nKey::Month => "Месяц",
        I18nKey::Day => "День",
        I18nKey::Publisher => "Издатель",
        I18nKey::SaveLiterature => "Сохранить",

        I18nKey::Field => "Поле",
        I18nKey::LocalData => "Локальные данные",
        I18nKey::RemoteData => "Удаленные данные",
        I18nKey::CompareAndMerge => "Сравнение и слияние",

        I18nKey::SubscriptionEditor => "Редактор подписок",
        I18nKey::FeedName => "Имя",
        I18nKey::FeedUrl => "URL",
        I18nKey::UpdateInterval => "Интервал обновления",
        I18nKey::Hours => "часов",
        I18nKey::SubscriptionNamePlaceholder => "Имя",
        I18nKey::SubscriptionUrlPlaceholder => "RSS URL",
        I18nKey::UpdateIntervalPlaceholder => "Интервал (в часах)",
        I18nKey::Add => "Добавить",

        I18nKey::SelectedSubscriptionCount => "Выбрано подписок: {}",
        I18nKey::BatchAddToLibrary => "Добавить в библиотеку ({})",
        I18nKey::AddedToLibrary => "Добавлено в библиотеку",
        I18nKey::AddToLibrary => "+ Добавить в библиотеку",
        I18nKey::FetchTime => "Время получения",
        I18nKey::NoSubscriptionSelected => "Подписка не выбрана",
        I18nKey::NoAbstract => "Нет аннотации",

        I18nKey::CopyCitationTitle => "Копировать цитату",
        I18nKey::Style => "Стиль",
        I18nKey::Preview => "Предпросмотр",
        I18nKey::NoLiteratureSelectedForCitation => "Литература не выбрана",
        I18nKey::CitationError => "Ошибка генерации цитаты",

        I18nKey::Type => "Тип",
        I18nKey::TypeArticle => "Статья",
        I18nKey::TypeBook => "Книга",
        I18nKey::TypeConference => "Материалы конференции",
        I18nKey::TypeThesis => "Диссертация",
        I18nKey::TypePreprint => "Препринт",
        I18nKey::TypeTechnicalReport => "Технический отчет",
        I18nKey::TypeWebpage => "Веб-страница",
        I18nKey::TypeOther => "Другое",

        I18nKey::TypeAll => "Вся литература",
        I18nKey::TypeCustom => "Пользовательская",
        I18nKey::TypeUncategorized => "Без категории",
        I18nKey::TypeTrash => "Корзина",

        I18nKey::Title => "Заголовок",
        I18nKey::Authors => "Авторы",
        I18nKey::Journal => "Журнал",
        I18nKey::Year => "Год",
        I18nKey::Volume => "Vol.",
        I18nKey::Issue => "No.",
        I18nKey::Pages => "Pages",
        I18nKey::Url => "URL",
        I18nKey::Doi => "DOI",
        I18nKey::ArXiv => "ArXiv",
        I18nKey::Abstract => "Аннотация",
        I18nKey::Keywords => "Ключевые слова",
        I18nKey::Notes => "Заметки",
        I18nKey::Attachments => "Вложения",
        I18nKey::Folders => "Папки",
        I18nKey::NoLiteratureSelected => "Литература не выбрана",
        I18nKey::SelectedCount => "Выбрано литературы: {}",
        I18nKey::MainFile => "Основной файл",
        I18nKey::Attachment => "Вложение",
        I18nKey::SetAsMainFile => "Сделать основным",
        I18nKey::SetAsAttachment => "Сделать вложением",
        I18nKey::Expand => "Развернуть всё ↓",
        I18nKey::Collapse => "Свернуть ↑",
        I18nKey::Publication => "Публикация",
        I18nKey::RelatedLiterature => "Связанная литература",
        I18nKey::AddCitation => "Добавить цитату",
        I18nKey::References => "Ссылки",
        I18nKey::CitedBy => "Цитируется",
        I18nKey::Settings => "Настройки",
        I18nKey::Language => "Язык",
        I18nKey::Appearance => "Внешний вид",
        I18nKey::UiScale => "Масштаб интерфейса",
        I18nKey::LogLevel => "Log Level",
        I18nKey::ThemeStyle => "Стиль темы",
        I18nKey::Theme => "Тема",
        I18nKey::Dark => "Темная",
        I18nKey::Light => "Светлая",
        I18nKey::System => "Системная",
        I18nKey::General => "Общие",
        I18nKey::Sync => "Синхронизация",
        I18nKey::About => "О программе",
        I18nKey::Cancel => "Отмена",
        I18nKey::Save => "Сохранить",
        I18nKey::LibrarySettings => "Настройки библиотеки",
        I18nKey::AttachmentDir => "Каталог вложений",
        I18nKey::AttachmentDirDesc => "Все PDF и вложения будут сохранены в этом каталоге",
        I18nKey::DatabaseDir => "Каталог базы данных",
        I18nKey::DatabaseDirDesc => "Где хранятся файлы базы данных",
        I18nKey::LogDir => "Каталог логов",
        I18nKey::LogDirDesc => "Место хранения лог-файлов приложения.",
        I18nKey::ThemeDir => "Каталог тем",
        I18nKey::ThemeDirDesc => "Каталог, в котором находятся JSON-файлы конфигурации тем.",
        I18nKey::FilenameTemplate => "Формат имени файла",
        I18nKey::FilenameTemplateDesc => {
            "Custom renaming rules for attachments. Available variables: {title}, {author}, {year}, {publication}, {firstname}, {lastname}, {firstchartitle}. Supports using '/' for folder hierarchy."
        }
        I18nKey::GeneralOptions => "Общие параметры",
        I18nKey::ThemeDesc => "Выберите цветовую тему интерфейса.",
        I18nKey::CloudSyncDesc => {
            "Облачная синхронизация находится в разработке. В будущем мы поддержим WebDAV, S3 и синхронизацию между устройствами."
        }
        I18nKey::AboutDesc => {
            "Высокопроизводительное приложение для управления литературой на базе GPUI. Чистый и мощный опыт чтения и исследований."
        }
        I18nKey::BatchRename => "Batch Rename",
        I18nKey::CleanupOrphanedFiles => "Cleanup Orphaned Files",
        I18nKey::Copyright => "© 2026 Lumen. Все права защищены.",
        I18nKey::AdvancedFilter => "Расширенный фильтр",
        I18nKey::Filter => "Фильтр",
        I18nKey::Reset => "Сбросить",
        I18nKey::YearStart => "Год с",
        I18nKey::YearEnd => "Год окончания",

        // Sort
        I18nKey::SortBy => "Сортировка",
        I18nKey::SortByTitle => "Название",
        I18nKey::SortByAuthor => "Автор",
        I18nKey::SortByYear => "Год",
        I18nKey::SortByJournal => "Журнал",
        I18nKey::SortAscending => "По возрастанию",
        I18nKey::SortDescending => "По убыванию",

        I18nKey::UpdatedAt => "Обновлено",

        // Sync
        I18nKey::SyncNow => "Синхронизировать сейчас",
        I18nKey::SyncMetadata => "Синхронизировать метаданные",
        I18nKey::SyncAttachments => "Синхронизировать вложения",
        I18nKey::TestConnection => "Проверить соединение",
        I18nKey::SyncSuccess => "Синхронизация завершена",
        I18nKey::SyncFailed => "Ошибка синхронизации",
        I18nKey::WebDavSettings => "Настройки WebDAV",
        I18nKey::DatabaseSettings => "Настройки базы данных",
        I18nKey::EndpointUrl => "Адрес сервера",
        I18nKey::Username => "Имя пользователя",
        I18nKey::Password => "Пароль",
        I18nKey::RemotePath => "Удаленный путь",
        I18nKey::Host => "Хост",
        I18nKey::Port => "Порт",
        I18nKey::DatabaseName => "Имя базы данных",
        I18nKey::EnableSSL => "Включить SSL",
        I18nKey::ConnectionSuccess => "Соединение установлено",
        I18nKey::ConnectionFailed => "Ошибка соединения",
        I18nKey::SearchOrCreateTags => "Найти или создать теги...",
        I18nKey::CreateTag => "Создать \"{}\"",
        I18nKey::AddTag => "Добавить тег",

        // PDF Viewer
        I18nKey::PdfViewerSettings => "Настройки PDF просмотрщика",
        I18nKey::PdfViewerSettingsDesc => "Настройте приложение для открытия PDF файлов",
        I18nKey::UseCustomPdfViewer => "Использовать собственный PDF просмотрщик",
        I18nKey::PdfViewerPath => "Путь к PDF просмотрщику",
        I18nKey::PdfViewerPathMacos => "Приложение macOS",
        I18nKey::PdfViewerPathWindows => "Программа Windows",
        I18nKey::PdfViewerPathPlaceholderMacos => "Напр.: /Applications/Skim.app",
        I18nKey::PdfViewerPathPlaceholderWindows => {
            "Напр.: C:\\Program Files\\SumatraPDF\\SumatraPDF.exe"
        }
        I18nKey::Browse => "Обзор",
        I18nKey::SelectMetadataCandidate => "Выберите наиболее подходящие метаданные",

        // Metadata Services
        I18nKey::MetadataServices => "Службы метаданных",
        I18nKey::EasyScholarKey => "Секретный ключ EasyScholar",
        I18nKey::EasyScholarDesc => "API-ключ для получения рейтингов журналов (JCR, CCF, CAS)",
        I18nKey::EasyScholarPlaceholder => "Введите секретный ключ EasyScholar...",
        I18nKey::OnDemandDownload => "On-Demand Download",
        I18nKey::OnDemandDownloadDesc => "...",

        // Service Errors
        I18nKey::LiteratureNotFound => "Литература не найдена",
        I18nKey::SubscriptionNotFound => "Подписка не найдена",
        I18nKey::FeedItemNotFound => "Элемент ленты не найден",
        I18nKey::LiteratureAddedNoRecord => "Литература добавлена, но запись не найдена",
        I18nKey::LiteratureNotFoundGeneric => "Литература не найдена",
        I18nKey::AttachmentNotFoundById => "Вложение не найдено: {}",
        I18nKey::FileNotFoundPath => "Файл не найден: {}",
        I18nKey::AttachmentNotFound => "Вложение не найдено",
        // Error/Notification
        I18nKey::FileNotFoundTitle => "Файл не найден",
        I18nKey::FileNotFoundMsg => "Путь {:?} не существует",
        I18nKey::DataConsistentTitle => "Данные совпадают",
        I18nKey::DataConsistentMsg => {
            "Полученные метаданные полностью совпадают с локальными данными. Слияние не требуется."
        }
        I18nKey::LiteratureMergedTitle => "Литература объединена",
        I18nKey::LiteratureMergedMsg => {
            "Дубликат \"{}\" идентичен основной записи и перемещён в корзину."
        }

        // Fetch Error Tips
        I18nKey::FetchFailedArxiv => {
            "У этой литературы нет ArXiv ID или связанной ссылки. Невозможно получить метаданные с ArXiv."
        }
        I18nKey::FetchFailedDblp => "Название литературы пусто. Невозможно выполнить поиск в DBLP.",
        I18nKey::FetchFailedCrossref => {
            "У этой литературы нет DOI или поле пусто. Невозможно получить метаданные с Crossref."
        }
        I18nKey::FetchFailedOpenAlex => {
            "DOI и название пусты. Невозможно выполнить поиск в OpenAlex."
        }

        // Batch Update
        I18nKey::BatchUpdatingMetadata => "Пакетное обновление метаданных ({}/{})",

        // Settings - Translation
        I18nKey::TranslationSettings => "Настройки перевода",
        I18nKey::TranslationSettingsDesc => {
            "Настройте движок перевода и API-ключи для PDF-читателя."
        }
        I18nKey::TranslationEngine => "Движок перевода",
        I18nKey::TranslationSettingsTab => "Перевод",
        I18nKey::NoApiKeyRequired => {
            "Этот движок не требует API-ключа и может использоваться напрямую."
        }
        I18nKey::NiuTransApiKey => "NiuTrans API Key",
        I18nKey::InternalReaderDesc => {
            "Когда внешний читатель отключён, PDF будет открываться во встроенном читателе"
        }
        I18nKey::SelectDirectory => "Выбрать {}",

        // PDF View - Notes
        I18nKey::EditNotesMarkdown => "Редактировать заметки (Markdown)",

        // Feed
        I18nKey::UnknownFeedSource => "Неизвестный источник ленты",

        // Bookmark
        I18nKey::UnnamedBookmark => "Закладка без названия",
        I18nKey::SelectMacosPdfReader => "Выберите программу просмотра PDF для macOS",
        I18nKey::SelectWindowsPdfReader => "Выберите программу просмотра PDF для Windows",

        // Pdf Viewer
        I18nKey::ToggleLeftSidebar => "Показать/скрыть боковую панель",
        I18nKey::SearchDocument => "Поиск в документе",
        I18nKey::RectangleSelect => "Прямоугольное выделение",
        I18nKey::ZoomOut => "Уменьшить",
        I18nKey::ZoomIn => "Увеличить",
        I18nKey::FitWidth => "По ширине",
        I18nKey::ToggleRightSidebar => "Переключить боковую панель",
        I18nKey::NotePlaceholder => "Введите текст заметки...",
        I18nKey::ViewNote => "Просмотр заметки",
        I18nKey::AddNote => "Добавить заметку",
        I18nKey::Highlight => "Выделение",
        I18nKey::Underline => "Подчёркивание",
        I18nKey::LoadingOutline => "Загрузка оглавления...",
        I18nKey::NoOutline => "У этого документа нет оглавления",
        I18nKey::RectangleAnnotation => "Прямоугольник",
        I18nKey::PageRange => "Стр. {}-{}",
        I18nKey::SinglePage => "Стр. {}",
        I18nKey::SelectTextToTranslate => "Выберите текст для перевода",
        I18nKey::OriginalSection => "Оригинал",
        I18nKey::TranslationSection => "Перевод",
        I18nKey::Translating => "Перевод...",
        I18nKey::TranslationPending => "Перевод ожидает",
        I18nKey::NoNotes => "Нет заметок",
        I18nKey::TranslateEngine => "Перевести",
        I18nKey::NiuTrans => "NiuTrans",
        I18nKey::Copy => "Копировать",
        I18nKey::PdfEngineError => "Ошибка движка рендеринга PDF",
        I18nKey::CloseWindow => "Закрыть окно",
        I18nKey::TranslationNotImplemented => "Перевод не реализован",

        // PDF Search
        I18nKey::SearchInPdf => "Поиск в PDF",
        I18nKey::NextMatch => "Следующее совпадение",
        I18nKey::PrevMatch => "Предыдущее совпадение",
        I18nKey::SearchInputPlaceholder => "Введите поисковый запрос...",
        I18nKey::NoSearchResults => "Нет результатов",
        I18nKey::SyncMetadataTab => "Синхронизация метаданных",
        I18nKey::SyncAttachmentTab => "Синхронизация вложений",
        I18nKey::GoogleDriveSettings => "Google Drive",
        I18nKey::GoogleDriveDesc => "Синхронизация вложений с Google Drive",
        I18nKey::ClientId => "ID клиента",
        I18nKey::ClientSecret => "Секрет клиента",
        I18nKey::Authorize => "Авторизовать",
        I18nKey::DataManagement => "Data Management",
        I18nKey::ClearLocalDb => "Clear Local Database",
        I18nKey::ClearLocalFiles => "Clear Local Files",
        I18nKey::ClearCloudDb => "Clear Cloud Database",
        I18nKey::ClearCloudFiles => "Clear Cloud Files",
    }
}
