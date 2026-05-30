use crate::I18nKey;

pub fn translate(key: I18nKey) -> &'static str {
    match key {
        I18nKey::Library => "Biblioteca",
        I18nKey::Subscription => "Suscripción",
        I18nKey::AllLiterature => "Toda la literatura",
        I18nKey::Uncategorized => "Sin clasificar",
        I18nKey::Trash => "Papelera",
        I18nKey::Tags => "Tags",
        I18nKey::AllSubscription => "Todas las suscripciones",
        I18nKey::Unread => "No leído",
        I18nKey::StatusReading => "Leyendo",
        I18nKey::StatusRead => "Leído",
        I18nKey::FolderNamePlaceholder => "Nombre de la carpeta",
        I18nKey::TagNamePlaceholder => "Nombre de la etiqueta",
        I18nKey::Version => "Versión",
        I18nKey::EmptyFolder => "La carpeta está vacía",
        I18nKey::NoMatchFound => "No se encontraron coincidencias",
        I18nKey::SearchBoxPlaceholder => "Buscar artículos, autores o revistas...",
        I18nKey::ManualAdd => "Añadir manualmente",
        I18nKey::BibTeXImport => "Importar BibTeX",
        I18nKey::DoiImport => "Importar DOI",
        I18nKey::ArXivImport => "Importar ArXiv",
        I18nKey::DblpSearch => "Buscar en DBLP",
        I18nKey::DuplicateGroups => "Duplicate Groups",
        I18nKey::SyncConflicts => "Version Conflicts",
        I18nKey::NoDuplicatesFound => "No duplicates found",
        I18nKey::NewFolder => "Nueva carpeta",
        I18nKey::EmptyTrash => "Vaciar papelera",
        I18nKey::Rename => "Renombrar",
        I18nKey::Delete => "Eliminar",
        I18nKey::NewSubscription => "Nueva suscripción",
        I18nKey::OpenInBrowser => "Abrir en el navegador",
        I18nKey::MarkAsRead => "Marcar como leído",
        I18nKey::MarkAsUnread => "Marcar como no leído",
        I18nKey::UpdateSubscription => "Actualizar",
        I18nKey::EditSubscription => "Editar suscripción",
        I18nKey::Unsubscribe => "Anular suscripción",
        I18nKey::AddSubscription => "Añadir suscripción",
        I18nKey::NewSubFolder => "Nueva subcarpeta",
        I18nKey::Edit => "Editar",
        I18nKey::Quit => "Salir",
        I18nKey::PermanentDelete => "Eliminar permanentemente",
        I18nKey::CopyCitation => "Copiar cita",
        I18nKey::FetchFrom => "Obtener de...",
        I18nKey::BatchFetchMetadata => "Actualización por lotes de metadatos",
        I18nKey::AddTo => "Añadir a...",
        I18nKey::RestoreTo => "Restaurar en...",
        I18nKey::RemoveFromFolder => "Quitar de la carpeta",
        I18nKey::RevealInFinder => "Mostrar en Finder",
        I18nKey::RevealInExplorer => "Mostrar en el Explorador",
        I18nKey::ReplaceFile => "Reemplazar archivo",
        I18nKey::DeleteFile => "Eliminar archivo",
        I18nKey::SelectNewFile => "Seleccionar nuevo archivo",
        I18nKey::Confirm => "Aceptar",
        I18nKey::LoadingMetadata => "Obteniendo metadatos...",
        I18nKey::FetchFailed => "Error al obtener",
        I18nKey::Retry => "Reintentar",
        I18nKey::Close => "Cerrar",
        I18nKey::ConfirmFetch => "Obtener ahora",
        I18nKey::FetchFromSource => "Obtener de {}",
        I18nKey::FetchPlaceholderDoi => "Introducir DOI",
        I18nKey::FetchPlaceholderArxiv => "Introducir ID de ArXiv",
        I18nKey::FetchPlaceholderBibtex => "Pegar BibTeX",
        I18nKey::FetchPlaceholderDblp => "Buscar en DBLP",
        I18nKey::FetchPlaceholderOpenAlex => "Buscar en OpenAlex",
        I18nKey::NoContentOrInvalidFormat => "Contenido vacío o formato no válido",
        I18nKey::ImportFailed => "Error de importación",

        I18nKey::LiteratureEditor => "Editor de literatura",
        I18nKey::AuthorPlaceholder => "Autores (separados por comas)",
        I18nKey::JournalPlaceholder => "Revista / Conferencia / Libro",
        I18nKey::Month => "Mes",
        I18nKey::Day => "Día",
        I18nKey::Publisher => "Editorial",
        I18nKey::Field => "Campo",
        I18nKey::LocalData => "Datos locales",
        I18nKey::RemoteData => "Datos remotos",
        I18nKey::SubscriptionEditor => "Editor de suscripciones",
        I18nKey::FeedName => "Nombre",
        I18nKey::FeedUrl => "URL",
        I18nKey::UpdateInterval => "Intervalo de actualización",
        I18nKey::SubscriptionNamePlaceholder => "Nombre",
        I18nKey::SubscriptionUrlPlaceholder => "URL RSS",
        I18nKey::UpdateIntervalPlaceholder => "Intervalo (horas)",
        I18nKey::Add => "Añadir",

        I18nKey::SelectedSubscriptionCount => "{} suscripciones seleccionadas",
        I18nKey::AddToLibrary => "+ Añadir a la biblioteca",
        I18nKey::NoSubscriptionSelected => "Ninguna suscripción seleccionada",
        I18nKey::NoAbstract => "No hay resumen disponible",

        I18nKey::CopyCitationTitle => "Copiar cita",
        I18nKey::Style => "Estilo",
        I18nKey::Preview => "Vista previa",
        I18nKey::NoLiteratureSelectedForCitation => "Ninguna literatura seleccionada",
        I18nKey::CitationError => "Error al generar cita",

        I18nKey::Type => "Tipo",
        I18nKey::TypeArticle => "Artículo de revista",
        I18nKey::TypeBook => "Libro",
        I18nKey::TypeConference => "Artículo de conferencia",
        I18nKey::TypeThesis => "Tesis",
        I18nKey::TypePreprint => "Preimpresión",
        I18nKey::TypeTechnicalReport => "Informe técnico",
        I18nKey::TypeWebpage => "Página web",
        I18nKey::TypeOther => "Otro",
        I18nKey::Title => "Título",
        I18nKey::Authors => "Autores",
        I18nKey::Journal => "Revista",
        I18nKey::Year => "Año",
        I18nKey::Volume => "Vol.",
        I18nKey::Issue => "No.",
        I18nKey::Pages => "Pages",
        I18nKey::Url => "URL",
        I18nKey::Doi => "DOI",
        I18nKey::ArXiv => "ArXiv",
        I18nKey::Abstract => "Resumen",
        I18nKey::Notes => "Notas",
        I18nKey::Attachments => "Adjuntos",
        I18nKey::Folders => "Carpetas",
        I18nKey::NoLiteratureSelected => "No hay literatura seleccionada",
        I18nKey::SelectedCount => "{} artículos seleccionados",
        I18nKey::MainFile => "Archivo principal",
        I18nKey::Attachment => "Adjunto",
        I18nKey::SetAsMainFile => "Establecer como principal",
        I18nKey::SetAsAttachment => "Establecer como adjunto",
        I18nKey::Expand => "Expandir todo ↓",
        I18nKey::Collapse => "Contraer ↑",
        I18nKey::Publication => "Publicación",
        I18nKey::RelatedLiterature => "Literatura relacionada",
        I18nKey::AddCitation => "Agregar cita",
        I18nKey::References => "Referencias",
        I18nKey::CitedBy => "Citado por",
        I18nKey::Settings => "Configuración",
        I18nKey::Language => "Idioma",
        I18nKey::Appearance => "Apariencia",
        I18nKey::UiScale => "Escala de UI",
        I18nKey::LogLevel => "Log Level",
        I18nKey::NotificationLevel => "Nivel de notificación",
        I18nKey::ThemeStyle => "Estilo de tema",
        I18nKey::Theme => "Tema",
        I18nKey::Dark => "Oscuro",
        I18nKey::Light => "Claro",
        I18nKey::System => "Sistema",
        I18nKey::General => "General",
        I18nKey::Sync => "Sincronización",
        I18nKey::About => "Acerca de",
        I18nKey::Cancel => "Cancelar",
        I18nKey::Save => "Guardar",
        I18nKey::LibrarySettings => "Ajustes de la biblioteca",
        I18nKey::AttachmentDir => "Directorio de adjuntos",
        I18nKey::AttachmentDirDesc => "Los PDF y adjuntos se guardarán aquí",
        I18nKey::DatabaseDir => "Directorio de la base de datos",
        I18nKey::DatabaseDirDesc => "Ubicación de los archivos de datos",
        I18nKey::FilenameTemplate => "Formato de nombre de archivo",
        I18nKey::FilenameTemplateDesc => {
            "Custom renaming rules for attachments. Available variables: {title}, {author}, {year}, {publication}, {firstname}, {lastname}, {firstchartitle}. Supports using '/' for folder hierarchy."
        }
        I18nKey::GeneralOptions => "Opciones generales",
        I18nKey::ThemeDesc => "Seleccione el tema de color.",
        I18nKey::CloudSyncDesc => "Sincronización en la nube en desarrollo.",
        I18nKey::AboutDesc => "Gestor de literatura de alto rendimiento basado en GPUI.",
        I18nKey::BatchRename => "Batch Rename",
        I18nKey::CleanupOrphanedFiles => "Cleanup Orphaned Files",
        I18nKey::Copyright => "© 2026 Lumen. Todos los derechos reservados.",
        // Sort
        I18nKey::SortBy => "Ordenar",
        I18nKey::SortByTitle => "Título",
        I18nKey::SortByAuthor => "Autor",
        I18nKey::SortByYear => "Año",
        I18nKey::SortByJournal => "Revista",
        I18nKey::SortAscending => "Ascendente",
        I18nKey::SortDescending => "Descendente",

        I18nKey::UpdatedAt => "Actualizado en",

        // Sync
        I18nKey::SyncMetadata => "Sincronizar metadatos",
        I18nKey::SyncAttachments => "Sincronizar adjuntos",
        I18nKey::TestConnection => "Probar conexión",
        I18nKey::WebDavSettings => "Ajustes de WebDAV",
        I18nKey::DatabaseSettings => "Ajustes de base de datos",
        I18nKey::EndpointUrl => "URL del servidor",
        I18nKey::Username => "Nombre de usuario",
        I18nKey::Password => "Contraseña",
        I18nKey::RemotePath => "Ruta remota",
        I18nKey::Host => "Host",
        I18nKey::Port => "Puerto",
        I18nKey::DatabaseName => "Nombre de la base de datos",
        I18nKey::EnableSSL => "Activar SSL",
        I18nKey::ConnectionSuccess => "Conexión exitosa",
        I18nKey::ConnectionFailed => "Conexión fallida",
        I18nKey::SearchOrCreateTags => "Buscar o crear etiquetas...",
        I18nKey::CreateTag => "Crear \"{}\"",

        // PDF Viewer
        I18nKey::PdfViewerSettings => "Configuración del visor PDF",
        I18nKey::PdfViewerSettingsDesc => "Personalizar la aplicación para abrir archivos PDF",
        I18nKey::UseCustomPdfViewer => "Usar visor PDF personalizado",
        I18nKey::PdfViewerPathMacos => "Aplicación macOS",
        I18nKey::PdfViewerPathWindows => "Programa Windows",
        I18nKey::SelectMetadataCandidate => "Seleccione los metadatos más coincidentes",

        // Metadata Services
        I18nKey::MetadataServices => "Servicios de metadatos",
        I18nKey::EasyScholarKey => "Clave secreta de EasyScholar",
        I18nKey::EasyScholarDesc => {
            "Clave API para obtener clasificaciones de revistas (JCR, CCF, CAS)"
        }
        I18nKey::EasyScholarPlaceholder => "Introduzca la clave secreta de EasyScholar...",
        I18nKey::OnDemandDownload => "On-Demand Download",
        I18nKey::OnDemandDownloadDesc => "...",

        // Service Errors
        // Error/Notification
        I18nKey::FileNotFoundTitle => "Archivo no encontrado",
        I18nKey::FileNotFoundMsg => "La ruta {:?} no existe",
        I18nKey::DataConsistentTitle => "Datos consistentes",
        I18nKey::DataConsistentMsg => {
            "Los metadatos obtenidos son idénticos a los datos locales. No es necesario fusionar."
        }
        I18nKey::LiteratureMergedTitle => "Literatura fusionada",
        I18nKey::LiteratureMergedMsg => {
            "El duplicado de \"{}\" es idéntico a la literatura principal y se ha movido a la papelera."
        }

        // Fetch Error Tips
        I18nKey::FetchFailedArxiv => {
            "Esta literatura no tiene ID de ArXiv ni enlace relacionado. No se pueden obtener metadatos de ArXiv."
        }
        I18nKey::FetchFailedDblp => {
            "El título de la literatura está vacío. No se puede buscar en DBLP."
        }
        I18nKey::FetchFailedCrossref => {
            "Esta literatura no tiene campo DOI o está vacío. No se pueden obtener metadatos de Crossref."
        }
        I18nKey::FetchFailedOpenAlex => {
            "Tanto el DOI como el título están vacíos. No se puede buscar en OpenAlex."
        }

        // Batch Update
        I18nKey::BatchUpdatingMetadata => "Actualización masiva de metadatos ({}/{})",

        // Settings - Translation
        I18nKey::TranslationSettings => "Configuración de traducción",
        I18nKey::TranslationSettingsDesc => {
            "Configure el motor de traducción y las claves API para el lector PDF."
        }
        I18nKey::TranslationEngine => "Motor de traducción",
        I18nKey::TranslationSettingsTab => "Traducción",
        I18nKey::NoApiKeyRequired => {
            "Este motor no requiere clave API y se puede usar directamente."
        }
        I18nKey::NiuTransApiKey => "Clave API de NiuTrans",
        I18nKey::GoogleApiKey => "Clave API de Google Cloud",
        I18nKey::BaiduApiKey => "Baidu AppID#Key",
        I18nKey::YoudaoApiKey => "Youdao AppID#Key",
        I18nKey::DeepLApiKey => "Clave API de DeepL",
        I18nKey::TargetLanguage => "Idioma de destino",
        I18nKey::EngineGoogleFree => "Google (gratuito)",
        I18nKey::EngineBingFree => "Bing (gratuito)",
        I18nKey::EngineGoogleCloud => "Google Cloud",
        I18nKey::EngineNiuTrans => "NiuTrans",
        I18nKey::EngineBaidu => "Baidu",
        I18nKey::EngineYoudao => "Youdao",
        I18nKey::EngineDeeplFree => "DeepL Free",
        I18nKey::EngineDeeplPro => "DeepL Pro",
        I18nKey::InternalReaderDesc => {
            "Cuando el lector externo está desactivado, el PDF se abrirá con el lector integrado"
        }
        // PDF View - Notes
        I18nKey::EditNotesMarkdown => "Editar notas (Markdown)",

        // Feed
        // Bookmark
        I18nKey::UnnamedBookmark => "Marcador sin nombre",
        I18nKey::SelectMacosPdfReader => "Seleccionar lector PDF de macOS",
        I18nKey::SelectWindowsPdfReader => "Seleccionar lector PDF de Windows",

        // Pdf Viewer
        I18nKey::ToggleLeftSidebar => "Mostrar/Ocultar barra lateral",
        I18nKey::RectangleSelect => "Selección rectangular",
        I18nKey::ZoomOut => "Alejar",
        I18nKey::ZoomIn => "Acercar",
        I18nKey::FitWidth => "Ajustar al ancho",
        I18nKey::ToggleRightSidebar => "Alternar barra lateral",
        I18nKey::NotePlaceholder => "Introduzca el contenido de la nota...",
        I18nKey::ViewNote => "Ver nota",
        I18nKey::AddNote => "Añadir nota",
        I18nKey::Highlight => "Resaltar",
        I18nKey::Underline => "Subrayar",
        I18nKey::LoadingOutline => "Cargando esquema...",
        I18nKey::NoOutline => "Este documento no tiene esquema",
        I18nKey::RectangleAnnotation => "Rectángulo",
        I18nKey::PageRange => "Página {}-{}",
        I18nKey::SinglePage => "Página {}",
        I18nKey::SelectTextToTranslate => "Seleccione texto para traducir",
        I18nKey::OriginalSection => "Original",
        I18nKey::TranslationSection => "Traducción",
        I18nKey::Translating => "Traduciendo...",
        I18nKey::TranslationPending => "Traducción pendiente",
        I18nKey::NoNotes => "Sin notas",
        I18nKey::Copy => "Copiar",
        I18nKey::PdfEngineError => "Error del motor de renderizado PDF",
        I18nKey::CloseWindow => "Cerrar ventana",
        I18nKey::TranslationNotImplemented => "Traducción no implementada",
        I18nKey::AutoTranslateOn => "Automático",
        I18nKey::AutoTranslateOff => "Manual",

        // PDF Search
        I18nKey::SearchInPdf => "Buscar en PDF",
        I18nKey::SearchInputPlaceholder => "Ingrese términos de búsqueda...",
        I18nKey::SyncMetadataTab => "Sincronización de metadatos",
        I18nKey::SyncAttachmentTab => "Sincronización de archivos adjuntos",
        I18nKey::GoogleDriveSettings => "Google Drive",
        I18nKey::GoogleDriveDesc => "Sincronizar archivos adjuntos a Google Drive",
        I18nKey::ClientId => "ID de cliente",
        I18nKey::ClientSecret => "Secreto de cliente",
        I18nKey::Authorize => "Autorizar",
        I18nKey::DataManagement => "Data Management",
        I18nKey::ClearLocalDb => "Clear Local Database",
        I18nKey::ClearLocalFiles => "Clear Local Files",
        I18nKey::ClearCloudDb => "Clear Cloud Database",
        I18nKey::ClearCloudFiles => "Clear Cloud Files",
    }
}
