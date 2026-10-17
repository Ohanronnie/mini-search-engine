use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use rust_stemmers::Stemmer;
use walkdir::WalkDir;
use xml::reader::{EventReader, XmlEvent};

use crate::{index::Indexer, rank::Ranker};
mod index;
mod rank;
mod tokenizer;

#[derive(Debug, Default)]
struct Document {
    id: usize,
    url: String,
    title: String,
    content: String,
}

#[derive(Debug)]
struct DocumentTokens {
    id: usize,
    url: String,
    title: String,
    title_tokenized: Vec<String>,
    tokens: Vec<String>,
    tokens_length: usize,
}
fn parse_document(path: PathBuf) -> Result<Vec<Document>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let file = BufReader::new(file);

    let parser = EventReader::new(file);
    let mut document = Document {
        ..Default::default()
    };
    let mut documents: Vec<Document> = Vec::new();
    for element in parser {
        match element? {
            XmlEvent::StartElement {
                name, attributes, ..
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
                        "title" => {
                            document.title = attribute.value;
                        }
                        _ => (),
                    }
                }
            }
            XmlEvent::Characters(value) => {
                document.content.push_str(&value);
            }
            XmlEvent::EndElement { .. } => {
                documents.push(document);
                document = Document {
                    ..Default::default()
                };
            }
            _ => (),
        }
    }

    Ok(documents)
}
fn stem_word(word: &str) -> String {
    let stemmer = Stemmer::create(rust_stemmers::Algorithm::English);

    stemmer.stem(word).to_string()
}
fn tokenize_text(text: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut collector = String::new();

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
            words.push(collector.to_owned());
            collector.clear();
        } else {
            collector.push(character);
        }
    }
    words.push(collector);
    let stemmed_words: Vec<String> = words.iter().map(|v| stem_word(v)).collect();
    stemmed_words
}
fn tokenize(document: &Document) -> DocumentTokens {
    let text = document.content.to_ascii_lowercase();
    let content_tokens = tokenize_text(&text);
    let title_tokenized = tokenize_text(&document.title.to_ascii_lowercase());
    DocumentTokens {
        id: document.id,
        url: document.url.clone(),
        title_tokenized: title_tokenized,
        title: document.title.clone(),
        tokens_length: content_tokens.len(),
        tokens: content_tokens,
    }
}

fn build_inverted_index(documents: &Vec<DocumentTokens>) -> HashMap<String, Vec<(usize, usize)>> {
    let mut inverted_index: HashMap<String, Vec<(usize, usize)>> = HashMap::new();

    for document in documents {
        // count unique tokens
        let mut unique: HashMap<String, usize> = HashMap::new();
        for token in &document.tokens {
            if let Some(v) = unique.get_mut(token) {
                *v += 1;
            } else {
                unique.insert(token.to_string(), 1);
            }
        }

        for (token, frequency) in unique {
            inverted_index
                .entry(token)
                .or_default()
                .push((document.id, frequency));
        }

        //   for token in &document.tokens {
        //      if let Some(v) = inverted_index.get_mut(token) {
        //        v[0].1 += 1;

        //     } else {
        //         inverted_index.insert(token.to_string(), vec![(document.id, 1)]);
        //     }
        // }
    }

    inverted_index
}
fn calculate_average_document_length(documents: &Vec<DocumentTokens>) -> f64 {
    if documents.is_empty() {
        return 0.0;
    }
    let result = documents
        .iter()
        .fold(0, |accumulator, value| accumulator + value.tokens_length);

    result as f64 / documents.len() as f64
}
fn term_frequency(
    token: &str,
    document_id: usize,
    index: &HashMap<String, Vec<(usize, usize)>>,
) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(index
        .get(token)
        .ok_or("not found")?
        .iter()
        .find(|(a, b)| *a == document_id)
        .ok_or("not found")?
        .1)
}
fn calculate_inverse_document_frequency(
    token: &str,
    documents: &Vec<DocumentTokens>,
    index: &HashMap<String, Vec<(usize, usize)>>,
) -> Result<f64, Box<dyn std::error::Error>> {
    let document_frequency = index.get(token).ok_or("not found")?.len() as f64;
    let document_count = documents.len() as f64;

    let idf = 1.0 + ((document_count - document_frequency + 0.5) / (document_frequency + 0.5));
    let idf = f64::ln(idf);

    Ok(idf)
}
fn calculate_bm25(idf: f64, tf: f64, dl: f64, avg_dl: f64) -> f64 {
    let k1 = 1.2;
    let b = 0.75;
    let result = idf * ((tf * (k1 + 1.0)) / (tf + (k1 * (1.0 - b + (b * (dl / avg_dl))))));

    result
}

fn search(
    word: &str,
    documents: &Vec<DocumentTokens>,
    average_dl: f64,
    index: &HashMap<String, Vec<(usize, usize)>>,
) -> Result<Vec<(String, f64)>, Box<dyn std::error::Error>> {
    let tokens = tokenize_text(word);

    let mut scores = HashMap::new();
    let mut unique = HashSet::new();
    for token in &tokens {
        if unique.insert(token.clone()) {
            for i in 0..documents.len() {
                let document = &documents[i];
                let idf = calculate_inverse_document_frequency(&token, &documents, index)?;
                let Ok(tf) = term_frequency(&token, document.id, index) else {
                    continue;
                };

                let bm = calculate_bm25(idf, tf as f64, document.tokens_length as f64, average_dl);
                let title_boost = tokens
                    .iter()
                    .filter(|v| document.title_tokenized.contains(v))
                    .count() as f64;

                *scores.entry(document.id).or_default() += bm + title_boost;
            }
        }
    }

    let mut collector: Vec<(String, f64)> = scores
        .into_iter()
        .map(|v| {
            let index = v.0;
            let value = documents
                .into_iter()
                .find(|doc| doc.id == index)
                .expect("should be found");
            (value.title.clone(), v.1)
        })
        .collect();
    collector.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
    collector.truncate(10);
    Ok(collector)
}
fn accept_input() -> std::io::Result<String> {
    let mut input = String::new();
    print!("Enter the keyword to search: ");
    std::io::stdout().flush()?;

    std::io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut indexer = Indexer::new();
    indexer.index(Path::new("wiki/extracted"))?;
    let ranker = Ranker::new(
        indexer.index,
        indexer.documents,
        indexer.average_document_len,
    );
    loop {
        let input = ranker.accept_input()?;
        if let Ok(result) = ranker.search(&input) {
            println!("{result:?}");
        } else {
            println!("not found")
        }
    }
    // let mut documents: Vec<Document> = Vec::new();
    // let walker = WalkDir::new("wiki/extracted");
    // for entry in walker {
    //     let entry = entry?;

    //     if entry.file_type().is_file() {
    //         let _doc = parse_document(entry.path().to_path_buf())?;
    //         documents.extend(_doc);
    //     }
    // }
    // //    let documents = parse_document("wiki/extracted/AA/wiki_00".into())?;
    // let tokenized_documents: Vec<DocumentTokens> = documents.iter().map(|v| tokenize(v)).collect();
    // drop(documents);
    // let inverted_index = build_inverted_index(&tokenized_documents);
    // let average_dl = calculate_average_document_length(&tokenized_documents);
    // loop {
    //     let input = accept_input()?;
    //     let result = search(
    //         &input.trim(),
    //         &tokenized_documents,
    //         average_dl,
    //         &inverted_index,
    //     )?;
    //     println!("{result:?}");
    // }

    Ok(())
}
