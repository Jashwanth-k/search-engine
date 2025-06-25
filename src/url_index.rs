use lazy_static::lazy_static;
use md5;
use std::sync::{Arc, RwLock};
use chrono::{self, DateTime, Utc};
use std::{env, fs};
use std::error::Error;
use std::io::Write;

#[derive(Clone)]
pub struct Node {
    url: String,
    pub hash: String,
    pub meta_content: String,
    left: Box<Option<Node>>,
    right: Box<Option<Node>>,
    pub timestamp: DateTime<Utc>,
}

lazy_static! {
    static ref root: Arc<RwLock<Option<Node>>> = Arc::new(RwLock::new(Option::None));
}

pub mod main {
    use super::*;
    pub fn index() -> Result<(), Box<dyn Error>>{
        let filepath = &env::var("URL_INDEX_FILE_PATH")?;
        let file_data = fs::read_to_string(filepath)?;
        let file_content: Vec<_> = file_data.lines().map(String::from).collect();
        for content in file_content {
            let content_data = content.split("$$==$$=$$").collect::<Vec<&str>>();
            let [url, content, meta_content]: [&str; 3] = content_data[..3].try_into().unwrap();
            insert(url, content, meta_content, false);
        }
        Ok(())
    }

    fn write_to_file(url: &str, content: &str, meta_content: &str) -> Result<(), Box<dyn Error>>{
        let filepath = &env::var("URL_INDEX_FILE_PATH")?;
        let mut file_data = fs::OpenOptions::new().create(true).append(true).open(filepath)?;
        let write_content = format!("{}$$==$$=$${}$$==$$=$${}\n", url, content, meta_content);
        let _ = file_data.write(write_content.as_bytes());
        Ok(())
    }

    pub fn get_hash(content: &str) -> String {
        let hash = md5::compute(content)
            .iter()
            .map(|x| x.to_string())
            .collect();
        return hash;
    }

    fn insert_helper(
        node: &mut Option<Node>,
        url: &str,
        content: &str,
        meta_content: &str,
    ) -> Option<Node> {
        if node.is_none() {
            let hash = get_hash(content);
            let new_node = Some(Node {
                url: String::from(url),
                hash: hash,
                meta_content: String::from(meta_content),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
                timestamp: chrono::Utc::now(),
            });
            return new_node;
        }
        let node = node.as_mut().unwrap();
        if node.url == url {
            node.meta_content = String::from(meta_content);
            node.hash = get_hash(content);
            node.timestamp = chrono::Utc::now();
            return Option::None;
        } else if *node.url >= *url {
            let resp = insert_helper(&mut node.right, url, content, meta_content);
            match resp {
                Some(next_node) => {
                    *node.right = Option::Some(next_node);
                    return Option::None;
                }
                _ => return Option::None,
            }
        } else {
            let resp = insert_helper(&mut node.right, url, content, meta_content);
            match resp {
                Some(next_node) => {
                    *node.left = Option::Some(next_node);
                    return Option::None;
                }
                _ => return Option::None,
            }
        }
    }

    pub fn insert(url: &str, content: &str, meta_content: &str, write_file: bool) {
        if write_file == true {
            let _ = write_to_file(url, content, meta_content);
        }
        println!("url_index insert triggered => text : {url}");
        let mut root_ref = root.write().unwrap();
        if root_ref.is_none() {
            let hash = get_hash(content);
            *root_ref = Some(Node {
                url: String::from(url),
                hash: hash,
                meta_content: String::from(meta_content),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
                timestamp: chrono::Utc::now(),
            });
            println!("root is updated");
            return;
        }
        insert_helper(&mut root_ref, url, content, meta_content);
    }

    fn get_helper(node: &Option<Node>, url: &str) -> Option<Node> {
        if node.is_none() {
            return Option::None;
        }
        let node = node.as_ref().unwrap();
        if node.url == url {
            return Option::Some(node.clone());
        } else if *node.url >= *url {
            return get_helper(&node.right, url);
        } else {
            return get_helper(&node.left, url);
        }
    }

    pub fn get_by_url(url: &str) -> Option<Node> {
        let root_ref = root.read().unwrap();
        return get_helper(&root_ref, url);
    }
}
