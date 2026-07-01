//! 数据转换模块
//!
//! 将 Literature 模型转换为 Hayagriva 的 Entry 格式

use hayagriva::{
    Entry,
    types::{Date, EntryType, MaybeTyped, Numeric, Person, Publisher, QualifiedUrl},
};
use log::debug;
use models::{Literature, LiteratureType, PublicationType};

/// 转换为 Hayagriva Entry
#[must_use]
pub fn literature_to_entry(lit: &Literature) -> Entry {
    debug!(
        "CSL 转换: 文献 '{}' (ID: {})",
        &lit.title[..lit.title.len().min(60)],
        lit.id
    );
    let mut entry = Entry::new(
        &lit.id,
        entry_type_from_literature_type(&lit.literature_type),
    );

    entry.set_title(lit.title.clone().into());

    let authors: Vec<Person> = lit
        .authors
        .iter()
        .map(|a| Person {
            name: a.last_name.clone(),
            given_name: Some(a.first_name.clone()),
            prefix: None,
            suffix: None,
            alias: None,
            comma_suffix: false,
        })
        .collect();
    if !authors.is_empty() {
        entry.set_authors(authors);
    }

    if let Some(year) = lit.year {
        let date = Date {
            year,
            month: lit.month.map(|m| m as u8),
            day: lit.day.map(|d| d as u8),
            approximate: false,
            season: None,
        };
        entry.set_date(date);
    }

    let container_title: Option<(String, EntryType)> = if let Some(ref pub_info) = lit.publication {
        let entry_type = match pub_info.publication_type {
            PublicationType::Journal => EntryType::Periodical,
            PublicationType::Conference => EntryType::Proceedings,
            PublicationType::Book => EntryType::Book,
        };
        Some((pub_info.name.clone(), entry_type))
    } else {
        None
    };

    if let Some((title, entry_type)) = container_title {
        let mut parent = Entry::new(&(lit.id.clone() + "-parent"), entry_type);
        parent.set_title(title.clone().into());
        entry.set_parents(vec![parent]);

        entry.set_keyed_serial_number("container-title", title);
    }

    if let Some(publisher_name) = lit.publication.as_ref().and_then(|p| p.publisher.clone()) {
        entry.set_publisher(Publisher::new(Some(publisher_name.into()), None));
    }

    if let Some(vol) = &lit.volume {
        if let Ok(num) = vol.parse::<i32>() {
            entry.set_volume(MaybeTyped::Typed(Numeric::new(num)));
        } else {
            entry.set_keyed_serial_number("volume", vol.clone());
        }
    }
    if let Some(issue) = &lit.issue {
        if let Ok(num) = issue.parse::<i32>() {
            entry.set_issue(MaybeTyped::Typed(Numeric::new(num)));
        } else {
            entry.set_keyed_serial_number("issue", issue.clone());
        }
    }
    if let Some(pages) = &lit.pages {
        entry.set_keyed_serial_number("page", pages.clone());
    }

    if let Some(doi) = &lit.doi {
        entry.set_doi(doi.clone());
    }
    if let Some(url) = &lit.url
        && let Ok(qualified_url) = url.parse::<QualifiedUrl>()
    {
        entry.set_url(qualified_url);
    }

    entry
}

fn entry_type_from_literature_type(lit_type: &LiteratureType) -> EntryType {
    match lit_type {
        LiteratureType::Article => EntryType::Article,
        LiteratureType::Book => EntryType::Book,
        LiteratureType::Conference => EntryType::Conference,
        LiteratureType::Thesis => EntryType::Thesis,
        LiteratureType::TechnicalReport => EntryType::Report,
        LiteratureType::Webpage => EntryType::Web,
        LiteratureType::Other | LiteratureType::Preprint => EntryType::Misc,
    }
}
