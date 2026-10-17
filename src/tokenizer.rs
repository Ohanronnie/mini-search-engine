use std::{collections::HashSet, sync::Arc};

use rust_stemmers::Stemmer;

use crate::index::{Document, DocumentTokens};

pub struct Tokenizer {
    stemmer: Stemmer,
}

impl Tokenizer {
    pub fn new() -> Self {
        Self {
            stemmer: Stemmer::create(rust_stemmers::Algorithm::English),
        }
    }

    pub fn tokenize_text(&self, text: &str) -> Arc<[Box<str>]> {
        let mut words = Vec::<Box<str>>::new();
        let mut collector = String::new();
        let text = text.trim().to_ascii_lowercase();
        let stop_words: HashSet<&str> = [
            "a", "an", "the", "and", "or", "but", "is", "am", "are", "was", "were", "be", "been",
            "being", "to", "of", "in", "on", "at", "for", "from", "by", "with", "as", "it", "its",
            "this", "that", "these", "those", "i", "me", "my", "you", "your", "he", "she", "we",
            "they", "his", "her", "our", "their", "them", "us", "do", "does", "did", "have", "has",
            "had",
        ]
        .into_iter()
        .collect();

        for character in text.chars() {
            if !character.is_alphanumeric() {
                if collector.len() < 1 {
                    continue;
                } else if stop_words.contains(&collector.as_str()) {
                    collector.clear();
                    continue;
                }
                words.push(std::mem::take(&mut collector).into_boxed_str());
            } else {
                collector.push(character);
            }
        }
        words.push(collector.into_boxed_str());

        let stemmed_words: Vec<Box<str>> = words
            .iter()
            .map(|v| self.stemmer.stem(v).to_string().into_boxed_str())
            .collect();

        stemmed_words.into()
    }
    pub fn tokenize(&self, document: Document) -> DocumentTokens {
        let text = document.content.to_ascii_lowercase();

        let content_tokens = self.tokenize_text(&text.to_ascii_uppercase().trim());
        drop(text);
        let title_tokens = self.tokenize_text(&document.title.trim());

        DocumentTokens {
            id: document.id,
            url: document.url.into(),
            title: document.title.into(),
            title_tokenized: title_tokens,
            tokens_length: content_tokens.len(),
            tokens: content_tokens,
        }
    }
}
