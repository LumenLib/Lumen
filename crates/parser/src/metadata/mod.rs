pub mod arxiv;
pub mod bibtex;
pub mod dblp;
pub mod doi;
pub mod openalex;

pub use arxiv::ArxivParser;
pub use bibtex::BibTeXParser;
pub use dblp::DblpParser;
pub use doi::DoiParser;
pub use openalex::OpenAlexParser;
