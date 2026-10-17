use crate::{
    index::{DocumentNew, InvertedIndex, Result},
    tokenizer::Tokenizer,
};
use std::io::Write;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub struct Ranker {
    index: InvertedIndex,
    documents: Vec<DocumentNew>,
    average_document_len: f64,
}

impl Ranker {
    pub fn new(
        index: InvertedIndex,
        documents: Vec<DocumentNew>,
        average_document_len: f64,
    ) -> Self {
        Self {
            index,
            documents,
            average_document_len,
        }
    }
    pub fn term_frequency(&self, document_id: u32, token: &str) -> Result<f64> {
        let value = self
            .index
            .get(token)
            .ok_or("Token not found in the index")?
            .iter()
            .find(|(a, _)| *a == document_id)
            .ok_or("Document not found how is that possible lol")?
            .1;

        Ok(value as f64)
    }

    fn calculate_inverse_document_frequency(&self, token: &str) -> Result<f64> {
        let document_frequency = self.index.get(token).ok_or("Token not found")?.len() as f64;
        let document_count = self.documents.len() as f64;
        let idf = 1.0 + ((document_count - document_frequency + 0.5) / (document_frequency + 0.5));
        let idf = f64::ln(idf);

        Ok(idf)
    }

    fn calculate_bm25(&self, idf: f64, tf: f64, dl: f64, avg_dl: f64) -> f64 {
        let k1 = 1.2;
        let b = 0.75;
        let result = idf * ((tf * (k1 + 1.0)) / (tf + (k1 * (1.0 - b + (b * (dl / avg_dl))))));

        result
    }

    pub fn search(&self, word: &str) -> Result<Vec<(Arc<[Box<str>]>, f64)>> {
        let tokens = Tokenizer::new().tokenize_text(word);

        let mut scores = HashMap::new();
        let mut unique = HashSet::new();

        for index in 0..tokens.len() {
            let token = &tokens[index];
            if unique.insert(token) {
                let doc_len = self.documents.len();
                for i in 0..doc_len {
                    let document = &self.documents[i];
                    let idf = self.calculate_inverse_document_frequency(&token)?;

                    let Ok(tf) = self.term_frequency(document.id, &token) else {
                        continue;
                    };

                    let bm = self.calculate_bm25(
                        idf,
                        tf,
                        document.length as f64,
                        self.average_document_len,
                    );
                    let title_boost =
                        tokens.iter().filter(|v| document.title.contains(v)).count() as f64;

                    *scores.entry(document.id).or_default() += bm + title_boost;
                }
            }
        }

        let mut collector: Vec<(Arc<[Box<str>]>, f64)> = scores
            .into_iter()
            .map(|v| {
                let index = v.0;

                let value = self
                    .documents
                    .iter()
                    .find(|doc| doc.id == index)
                    .expect("Should be found");

                (Arc::clone(&value.title), v.1)
            })
            .collect();
        collector.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        collector.truncate(10);
        Ok(collector)
    }
    pub fn accept_input(&self) -> Result<String> {
        let mut input = String::new();
        print!("Enter the keyword to search: ");
        std::io::stdout().flush()?;

        std::io::stdin().read_line(&mut input)?;

        Ok(input.trim().to_string())
    }
}
