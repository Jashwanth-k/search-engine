use crate::url_index;
use float_ord::FloatOrd;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::io::Write;
use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};
use std::{env, error::Error, fs};

pub struct Node {
    text: String,
    urls: HashSet<String>,
    left: Box<Option<Node>>,
    right: Box<Option<Node>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResultScore {
    url: String,
    title: String,
    score: f64,
}

lazy_static! {
    static ref root: Arc<RwLock<Option<Node>>> = Arc::new(RwLock::new(Option::None));
}

pub mod main {
    use super::*;
    pub fn index() -> Result<(), Box<dyn Error>> {
        let filepath = &env::var("URL_INDEX_FILE_PATH")?;
        let file_data = fs::read_to_string(filepath)?;
        let file_content: Vec<_> = file_data.lines().map(String::from).collect();
        for content in file_content {
            let content_data = content.split("$$==$$=$$").collect::<Vec<&str>>();
            match content_data.len() {
                5 => (),
                _ => continue,
            }
            let [url, title, headings, highlighted, content]: [&str; 5] =
                content_data[..5].try_into().unwrap();
            // println!("{url} == {content} == {title} == {headings} == {highlighted}");
            insert_by_content(url, content, title, headings, highlighted);
        }
        Ok(())
    }

    fn write_to_file(url: &str, content: &str) -> Result<(), Box<dyn Error>> {
        let filepath = &env::var("INVERTED_INDEX_FILE_PATH")?;
        let mut file_data = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(filepath)?;
        let write_content = format!("{}$$==$$=$${}\n", url, content);
        let _ = file_data.write(write_content.as_bytes());
        Ok(())
    }

    fn insert_helper(node: &mut Option<Node>, text: &str, url: &str) -> Option<Node> {
        if node.is_none() {
            let new_node = Some(Node {
                text: String::from(text),
                urls: HashSet::from([String::from(url)]),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
            });
            return new_node;
        }
        let node = node.as_mut().unwrap();
        if node.text == text {
            node.urls.insert(url.to_string());
            return Option::None;
        } else if *node.text >= *text {
            let resp = insert_helper(&mut node.right, text, url);
            match resp {
                Some(next_node) => {
                    *node.right = Option::Some(next_node);
                    return Option::None;
                }
                _ => return Option::None,
            }
        } else {
            let resp = insert_helper(&mut node.left, text, url);
            match resp {
                Some(next_node) => {
                    *node.left = Option::Some(next_node);
                    return Option::None;
                }
                _ => return Option::None,
            }
        }
    }

    pub fn insert_by_content(
        url: &str,
        content: &str,
        title: &str,
        headings: &str,
        highlighted: &str,
    ) {
        println!("inverted_index insert triggered => url : {url}");
        let whole_content = format!("{} {} {} {}", title, headings, highlighted, content);
        let words_map = whole_content.split_whitespace();
        for word in words_map {
            // println!("🔥🔥 word : {word} ==== url : {url}");
            insert(word, url);
        }
    }

    pub fn insert(text: &str, url: &str) {
        let text = &text.to_string().to_lowercase();
        let mut root_ref = root.write().unwrap();
        if root_ref.is_none() {
            *root_ref = Some(Node {
                text: String::from(text),
                urls: HashSet::from([String::from(url)]),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
            });
            println!("root is updated");
            return;
        }
        insert_helper(&mut root_ref, text, url);
    }

    fn get_helper(node: &Option<Node>, text: &str) -> Option<Vec<String>> {
        if node.is_none() {
            return Option::None;
        }
        let node = node.as_ref().unwrap();
        if node.text == text {
            // return Some(node.urls_map.clone());
            return Some(Vec::from_iter(node.urls.clone()));
        } else if *node.text >= *text {
            return get_helper(&node.right, text);
        } else {
            return get_helper(&node.left, text);
        }
    }

    pub fn get_by_text(text: &str) -> Option<Vec<String>> {
        let text = text.to_string().to_lowercase();
        let root_ref = root.read().unwrap();
        let mut combined_result = Vec::<String>::new();
        for word in text.split_whitespace() {
            if let Some(mut word_result) = get_helper(&root_ref, word) {
                // println!("word: {word}, map: {:?}", word_result);
                combined_result.append(&mut word_result);
            }
        }
        // println!("hashmap result {:?}", combined_result);
        Some(combined_result)
    }

    fn get_bm25_score(f_q_d: u64, d: u64, avdl: u64, n: u64, n_q: u64) -> f64 {
        // f_q_d no of times query q occurs in doc
        // n_q no of docs containing query q
        // d length of doc
        // avdl average document len across collection
        // n total no of docs
        // k how quickly the term frequency score saturates, inversely propotional to score saturation progress
        // // high k -> query occurance of 5 times will result in 90% score
        // // low k -> queyr occurance of 5 times will result in 70% score
        // b contorls strength of the penalty for long documents
        // idf give more weight to rare words than most repeated ones

        let k = 1.2;
        let b = 1 as f64;
        let n_q = n_q as f64;
        let f_q_d = f_q_d as f64;
        let d = d as f64;
        let avdl = avdl as f64;
        let n = n as f64;
        let idf = (((n - n_q + 0.5) / (n_q + 0.5)) + 1.0).ln();
        let tf_satur = (f_q_d * (k + 1.0)) / (f_q_d + k * (1.0 - b + b * (d / avdl)));
        let rank = idf * tf_satur;
        rank
    }

    fn get_score_helper(
        weight: u8,
        document: &String,
        query: &str,
        total_doc_len: u64,
        total_doc_count: u64,
        total_query_count_doc: u64,
    ) -> f64 {
        let doc_len = document.len() as u64;
        let avg_content_len = total_doc_len / total_doc_count;
        let mut occurance_count: u64 = 0;
        for word in document.split_whitespace() {
            if word == query {
                occurance_count += 1
            }
        }
        weight as f64
            * get_bm25_score(
                occurance_count,
                doc_len,
                avg_content_len,
                total_doc_count,
                total_query_count_doc,
            )
    }

    pub fn get_text_by_scoring(text: &str) -> Result<Vec<ResultScore>, Box<dyn Error>> {
        let top_k = &env::var("TOP_K")
            .unwrap_or(String::from("10"))
            .parse::<u8>()
            .unwrap();
        let text = text.to_string().to_lowercase();
        let root_ref = root.read().unwrap();
        let mut combines_map = HashMap::<String, (String, f64)>::new();
        for word in text.split_whitespace() {
            let word_urls = get_helper(&root_ref, word);
            if word_urls.is_none() {
                continue;
            }
            let word_urls = word_urls.unwrap();
            let word_freq = word_urls.len() as u64;
            for word_url in word_urls {
                let data_node = url_index::main::get_by_url(&word_url);
                if data_node.is_none() {
                    continue;
                }
                let data_node = data_node.unwrap();
                let url_index::Node {
                    title,
                    headings,
                    highlighted,
                    content,
                    ..
                } = data_node;
                let curr_index_config = url_index::INDEX_CONFIG.clone();
                let curr_index_config = curr_index_config.read().unwrap();
                let curr_score = get_score_helper(
                    6,
                    &title,
                    word,
                    curr_index_config.field_count.title,
                    curr_index_config.total_count,
                    word_freq,
                ) + get_score_helper(
                    4,
                    &headings,
                    word,
                    curr_index_config.field_count.headings,
                    curr_index_config.total_count,
                    word_freq,
                ) + get_score_helper(
                    2,
                    &highlighted,
                    word,
                    curr_index_config.field_count.highlighted,
                    curr_index_config.total_count,
                    word_freq,
                ) + get_score_helper(
                    1,
                    &content,
                    word,
                    curr_index_config.field_count.content,
                    curr_index_config.total_count,
                    word_freq,
                );
                combines_map
                    .entry(word_url.to_string())
                    .and_modify(|entry| {
                        entry.1 += curr_score;
                    })
                    .or_insert((title.to_string(), curr_score));
            }
        }

        let mut heap = BinaryHeap::<(FloatOrd<f64>, String, String)>::new();
        for (url, (title, score)) in combines_map {
            heap.push((FloatOrd(-score), url, title));
            if heap.capacity() as u64 > *top_k as u64 {
                heap.pop();
            }
        }
        let final_result = heap
            .into_sorted_vec()
            .into_iter()
            .map(|(score, url, title)| ResultScore {
                score: -score.0,
                url,
                title,
            })
            .collect::<Vec<ResultScore>>();
        Ok(final_result)
    }
}
