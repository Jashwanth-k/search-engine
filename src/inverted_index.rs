use crate::url_index;
use float_ord::FloatOrd;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::cmp;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::thread;
use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};
use std::{env, error::Error, fs, fs::File};

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
        let file_data = File::open(filepath);
        if file_data.is_err() {
            println!("err while loading index : {:?}", file_data);
            return Ok(());
        }
        let file_data = file_data.unwrap();
        let reader = BufReader::new(file_data);
        for line in reader.lines() {
            if line.is_err() {
                println!("buffer read line error : {:?}", line);
                continue;
            }
            let content = line.unwrap();
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
        println!("=== INVERTED INDEXING FINISHED ===");
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
        let rank = cmp::max(FloatOrd(idf), FloatOrd(0.0)).0 * cmp::max(FloatOrd(tf_satur), FloatOrd(0.0)).0;
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
        let score = weight as f64
            * get_bm25_score(
                occurance_count,
                doc_len,
                avg_content_len,
                total_doc_count,
                total_query_count_doc,
            );
        score
    }

    fn get_text_by_scoring_helper(
        text: &str,
    ) -> Result<Vec<ResultScore>, Box<dyn Error>> {
        let text = text.to_string().to_lowercase();
        let combined_map = Arc::new(RwLock::new(HashMap::<String, (String, f64, f64)>::new()));
        let mut all_handles = Vec::new();
        for word in text.split_whitespace() {
            let word = word.to_string();
            let combined_map = Arc::clone(&combined_map);
            let mut sub_handles = Vec::new();
            let root_ref = Arc::clone(&root);
            let handle = thread::spawn(move || {
                let root_ref = root_ref.read().unwrap();
                let word_urls = get_helper(&root_ref, &word);
                if word_urls.is_none() {
                    return;
                }
                let word_urls = word_urls.unwrap();
                let word_freq = word_urls.len() as u64;
                for word_url in word_urls {
                    let word = word.to_string();
                    let combined_map = Arc::clone(&combined_map);
                    let sub_handle = thread::spawn(move || {
                        let mut combined_map = combined_map.write().unwrap();
                        let data_node = url_index::main::get_by_url(&word_url);
                        if data_node.is_none() {
                            return;
                        }
                        let data_node = data_node.unwrap();
                        let url_index::Node {
                            title,
                            headings,
                            highlighted,
                            content,
                            ..
                        } = data_node;
                        let curr_index_config = &url_index::INDEX_CONFIG;
                        let curr_index_config = curr_index_config.read().unwrap();
                        let url_score = get_score_helper(
                            8,
                            &word_url,
                            &word,
                            curr_index_config.field_count.url,
                            curr_index_config.total_count,
                            word_freq,
                        );
                        let title_score = get_score_helper(
                            6,
                            &title,
                            &word,
                            curr_index_config.field_count.title,
                            curr_index_config.total_count,
                            word_freq,
                        );
                        let headings_score = get_score_helper(
                            4,
                            &headings,
                            &word,
                            curr_index_config.field_count.headings,
                            curr_index_config.total_count,
                            word_freq,
                        );
                        let highlighted_score = get_score_helper(
                            2,
                            &highlighted,
                            &word,
                            curr_index_config.field_count.highlighted,
                            curr_index_config.total_count,
                            word_freq,
                        );
                        let content_score = get_score_helper(
                            1,
                            &content,
                            &word,
                            curr_index_config.field_count.content,
                            curr_index_config.total_count,
                            word_freq,
                        );
                        let curr_score = url_score + title_score + headings_score + highlighted_score + content_score;
                        combined_map
                            .entry(word_url.to_string())
                            .and_modify(|entry| {
                                entry.1 += curr_score;
                                entry.2 += 1.0;
                            })
                            .or_insert((title.to_string(), curr_score, 1.0));
                    });
                    sub_handles.push(sub_handle);
                }
                for handle in sub_handles {
                    let _ = handle.join();
                }
            });
            all_handles.push(handle);
        }
        for handle in all_handles {
            let _ = handle.join();
        }
        let combined_map = Arc::try_unwrap(combined_map).unwrap().into_inner().unwrap();
        let final_result = combined_map
            .into_iter()
            .map(|(url, (title, score, freq))| ResultScore {
                score: score * freq, // boosting score for pages which has entire search text
                url: url.to_string(),
                title: title.to_string(),
            })
            .collect::<Vec<ResultScore>>();
        Ok(final_result)
    }

    pub fn get_text_by_scoring(text: &str) -> Result<Vec<ResultScore>, Box<dyn Error>> {
        let top_k: u8 = env::var("TOP_K_RESULTS")
            .unwrap_or(String::from("10"))
            .parse::<u8>()
            .unwrap();
        let result = get_text_by_scoring_helper(text)?;
        let top_results = get_top_k_filterd(result, top_k)?;
        Ok(top_results)
    }

    fn get_top_k_filterd(
        url_results: Vec<ResultScore>,
        top_k: u8,
    ) -> Result<Vec<ResultScore>, Box<dyn Error>> {
        let mut heap = BinaryHeap::new();
        // println!("raw result 🥺🥺: {:#?}", url_results);
        for ResultScore { url, title, score } in url_results.iter() {
            heap.push((FloatOrd(-score), url, title));
            if heap.len() as u64 > top_k as u64 {
                heap.pop();
            }
        }
        // println!("heap 🥺🥺 {top_k} : {:#?}", heap);
        let final_result = heap
            .into_sorted_vec()
            .into_iter()
            .map(|(score, url, title)| ResultScore {
                score: -score.0,
                url: url.to_string(),
                title: title.to_string(),
            })
            .collect::<Vec<ResultScore>>();
        Ok(final_result)
    }
}
