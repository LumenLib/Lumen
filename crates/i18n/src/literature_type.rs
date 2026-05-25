use crate::I18nKey;
use models::LiteratureType;

pub trait LiteratureTypeExt {
    fn i18n_key(&self) -> I18nKey;
    fn all() -> Vec<LiteratureType>;
}

impl LiteratureTypeExt for LiteratureType {
    fn i18n_key(&self) -> I18nKey {
        match self {
            LiteratureType::Article => I18nKey::TypeArticle,
            LiteratureType::Book => I18nKey::TypeBook,
            LiteratureType::Conference => I18nKey::TypeConference,
            LiteratureType::Thesis => I18nKey::TypeThesis,
            LiteratureType::Preprint => I18nKey::TypePreprint,
            LiteratureType::TechnicalReport => I18nKey::TypeTechnicalReport,
            LiteratureType::Webpage => I18nKey::TypeWebpage,
            LiteratureType::Other => I18nKey::TypeOther,
        }
    }

    fn all() -> Vec<Self> {
        vec![
            Self::Article,
            Self::Book,
            Self::Conference,
            Self::Thesis,
            Self::Preprint,
            Self::TechnicalReport,
            Self::Webpage,
            Self::Other,
        ]
    }
}
