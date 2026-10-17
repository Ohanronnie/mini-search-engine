use std::{
    collections::{HashMap, HashSet},
    fs::File,
    hash::Hash,
    io::BufReader,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use walkdir::WalkDir;
use xml::{EventReader, reader::XmlEvent};

use crate::tokenizer::Tokenizer;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub type InvertedIndex = HashMap<Box<str>, Vec<(u32, u32)>>;

#[derive(Debug, Default)]
pub struct Document {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Default)]
pub struct DocumentTokens {
    pub id: u32,
    pub url: Arc<str>,
    pub title: Arc<str>,
    pub title_tokenized: Arc<[Box<str>]>,
    pub tokens: Arc<[Box<str>]>,
    pub tokens_length: usize,
}

#[derive(Debug, Default)]
pub struct DocumentNew {
    pub id: u32,
    pub url: Arc<str>,
    pub title: Arc<[Box<str>]>,
    pub length: u32,
}

#[derive(Default)]
pub struct Indexer {
    pub index: InvertedIndex,
    pub documents: Vec<DocumentNew>,
    pub average_document_len: f64,
}
impl Indexer {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn index(&mut self, file_path: &Path) -> Result<()> {
        let tokenizer = Tokenizer::new();
        let timer = Instant::now();
        let walkdir = WalkDir::new(file_path);
        let mut total = 0;
        let mut doc_length = 0;
        for entry in walkdir {
            let entry = entry?;
            if entry.file_type().is_dir() {
                continue;
            }
            let file_name = entry.path();

            for document in self.parse_document(file_name.into())? {
                total += 1;
                let document = tokenizer.tokenize(document);
                self.documents.push(DocumentNew {
                    id: document.id,
                    url: Arc::clone(&document.url),
                    title: Arc::clone(&document.title_tokenized),
                    length: document.tokens_length as u32,
                });
                doc_length += document.tokens_length;
                self.build_inverted_index(document);
            }
        }
        self.average_document_len = doc_length as f64 / total as f64;
        println!("{:.3?}", timer.elapsed());

        Ok(())
    }
    fn parse_document(&self, path: &Path) -> Result<Vec<Document>> {
        let file = File::open(path)?;
        let file = BufReader::new(file);

        let parser = EventReader::new(file);
        let mut document = Document::default();
        let mut documents = Vec::<Document>::new();

        for element in parser {
            match element? {
                XmlEvent::StartElement {
                    name,
                    attributes,
                    namespace,
                } => {
                    if name.local_name != "doc" {
                        return Err("Invalid document".into());
                    }
                    for attribute in attributes {
                        match attribute.name.local_name.as_str() {
                            "id" => {
                                document.id = attribute.value.parse()?;
                            }
                            "url" => {
                                document.url = attribute.value;
                            }
                            "title" => document.title = attribute.value,

                            _ => (),
                        }
                    }
                }
                XmlEvent::Characters(value) => {
                    document.content.push_str(&value);
                }
                XmlEvent::EndElement { .. } => {
                    documents.push(document);
                    document = Document::default();
                }
                _ => (),
            }
        }
        Ok(documents)
    }

    fn build_inverted_index(&mut self, document: DocumentTokens) {
        let mut unique = HashMap::new();

        for token in document.tokens.iter() {
            if let Some(v) = unique.get_mut(&token) {
                *v += 1;
            } else {
                unique.insert(token, 1);
            }
        }

        for (token, frequency) in unique {
            self.index
                .entry(token.clone())
                .or_default()
                .push((document.id, frequency));
        }
    }
}
