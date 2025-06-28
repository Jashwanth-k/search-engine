use lazy_static::lazy_static;
use std::io::Write;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use std::{env, error::Error, fs};

pub struct Node {
    text: String,
    urls_map: HashMap<String, u64>,
    left: Box<Option<Node>>,
    right: Box<Option<Node>>,
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
            let [url, content, meta_content]: [&str; 3] = content_data[..3].try_into().unwrap();
            insert_by_content(url, content);
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

    fn insert_helper(node: &mut Option<Node>, text: &str, url: &str, count: u64) -> Option<Node> {
        if node.is_none() {
            let new_node = Some(Node {
                text: String::from(text),
                urls_map: HashMap::from([(String::from(url), count)]),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
            });
            return new_node;
        }
        let node = node.as_mut().unwrap();
        if node.text == text {
            node.urls_map.entry(url.to_string()).or_insert(count);
            return Option::None;
        } else if *node.text >= *text {
            let resp = insert_helper(&mut node.right, text, url, count);
            match resp {
                Some(next_node) => {
                    *node.right = Option::Some(next_node);
                    return Option::None;
                }
                _ => return Option::None,
            }
        } else {
            let resp = insert_helper(&mut node.left, text, url, count);
            match resp {
                Some(next_node) => {
                    *node.left = Option::Some(next_node);
                    return Option::None;
                }
                _ => return Option::None,
            }
        }
    }

    pub fn insert_by_content(url: &str, content: &str) {
        let words_map = content
            .split_whitespace()
            .fold(HashMap::new(), |mut acc, word| {
                *acc.entry(word).or_default() += 1;
                return acc;
            });
        for (word, count) in words_map {
            insert(word, url, count);
        }
    }

    pub fn insert(text: &str, url: &str, count: u64) {
        let text = &text.to_string().to_lowercase();
        println!("inverted_index insert triggered => text : {text}, url : {url}");
        let mut root_ref = root.write().unwrap();
        if root_ref.is_none() {
            *root_ref = Some(Node {
                text: String::from(text),
                urls_map: HashMap::from([(String::from(url), count)]),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
            });
            println!("root is updated");
            return;
        }
        insert_helper(&mut root_ref, text, url, count);
    }

    fn get_helper(node: &Option<Node>, text: &str) -> Option<HashMap<String, u64>> {
        if node.is_none() {
            return Option::None;
        }
        let node = node.as_ref().unwrap();
        if node.text == text {
            return Some(node.urls_map.clone());
            // return Option::Some(Vec::from_iter(node.urls_set.clone()));
        } else if *node.text >= *text {
            return get_helper(&node.right, text);
        } else {
            return get_helper(&node.left, text);
        }
    }

    pub fn get_by_text(text: &str) -> Option<HashMap<String, u64>> {
        let text = text.to_string().to_lowercase();
        let root_ref = root.read().unwrap();
        let mut combined_result = HashMap::<String, u64>::new();
        for word in text.split_whitespace() {
            if let Some(word_result) = get_helper(&root_ref, word) {
                // println!("word: {word}, map: {:?}", word_result);
                for (w, ct) in word_result {
                    *combined_result.entry(w).or_default() += ct;
                }
            }
        }
        // println!("hashmap result {:?}", combined_result);
        Some(combined_result)
    }
}
