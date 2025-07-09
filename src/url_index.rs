use chrono::{self, DateTime, Utc};
use lazy_static::lazy_static;
use md5;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, RwLock};
use std::{env, fs};

#[derive(Clone)]
pub struct Node {
    url: String,
    pub hash: String,
    pub title: String,
    pub headings: String,
    pub highlighted: String,
    pub content: String,
    left: Box<Option<Node>>,
    right: Box<Option<Node>>,
    pub timestamp: DateTime<Utc>,
}

pub struct FieldCount {
    pub url: u64,
    pub title: u64,
    pub headings: u64,
    pub highlighted: u64,
    pub content: u64,
}

pub struct IndexConfig {
    pub total_count: u64,
    pub field_count: FieldCount,
}

lazy_static! {
    static ref root: Arc<RwLock<Option<Node>>> = Arc::new(RwLock::new(Option::None));
    pub static ref INDEX_CONFIG: Arc<RwLock<IndexConfig>> = Arc::new(RwLock::new(IndexConfig {
        total_count: 0,
        field_count: FieldCount {
            url: 0,
            title: 0,
            headings: 0,
            highlighted: 0,
            content: 0,
        }
    }));
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
            insert(url, content, title, headings, highlighted);
        }
        Ok(())
    }

    fn traverse_and_write(
        node: &Option<Node>,
        mut file: &File,
    ) -> Result<(), Box<dyn Error + Send>> {
        if node.is_none() {
            return Ok(());
        }
        let node = node.as_ref().unwrap();
        let url = &node.url;
        let content = &node.content;
        let title = &node.title;
        let headings = &node.headings;
        let highlighted = &node.highlighted;
        let write_content = format!(
            "{}$$==$$=$${}$$==$$=$${}$$==$$=$${}$$==$$=$${}\n",
            url, title, headings, highlighted, content
        );
        let _ = file.write(write_content.as_bytes());
        let _ = traverse_and_write(&node.right, &file);
        let _ = traverse_and_write(&node.left, &file);
        Ok(())
    }

    pub fn write_to_file() -> Result<(), Box<dyn Error + Send + Sync>> {
        println!("writing index to file");
        let filepath = &env::var("URL_INDEX_FILE_PATH")?;
        let temp_filepath = String::from(filepath).replace(".txt", "-temp.txt");
        let file_data = File::create(&temp_filepath)?;
        let root_clone = root.clone();
        let root_ref = root_clone.read().unwrap();
        let _ = traverse_and_write(&root_ref, &file_data);
        let _ = fs::rename(&temp_filepath, filepath);
        Ok(())
    }

    pub fn get_hash(content: &str) -> String {
        let hash = md5::compute(content)
            .iter()
            .map(|x| x.to_string())
            .collect();
        return hash;
    }

    pub fn calc_helper(old_value: u64, curr_value: u64, count: u64) -> u64 {
        old_value * ((count - 1) / count) + (curr_value / count)
    }

    pub fn handle_index_config_update(
        url: &str,
        content: &str,
        title: &str,
        headings: &str,
        highlighted: &str,
    ) {
        let mut curr_index_config = INDEX_CONFIG.write().unwrap();
        curr_index_config.total_count += 1;
        curr_index_config.field_count.url += url.len() as u64;
        curr_index_config.field_count.content += content.len() as u64;
        curr_index_config.field_count.title += title.len() as u64;
        curr_index_config.field_count.headings += headings.len() as u64;
        curr_index_config.field_count.highlighted += highlighted.len() as u64;
    }

    fn insert_helper(
        node: &mut Option<Node>,
        url: &str,
        content: &str,
        title: &str,
        headings: &str,
        highlighted: &str,
    ) -> Option<Node> {
        if node.is_none() {
            let hash = get_hash(content);
            let new_node = Some(Node {
                url: String::from(url),
                hash: hash,
                content: String::from(content),
                title: String::from(title),
                headings: String::from(headings),
                highlighted: String::from(highlighted),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
                timestamp: chrono::Utc::now(),
            });
            return new_node;
        }
        let node = node.as_mut().unwrap();
        if node.url == url {
            node.title = String::from(title);
            node.headings = String::from(headings);
            node.highlighted = String::from(highlighted);
            node.hash = get_hash(content);
            node.timestamp = chrono::Utc::now();
            return Option::None;
        } else if *url >= *node.url {
            let resp = insert_helper(&mut node.right, url, content, title, headings, highlighted);
            match resp {
                Some(next_node) => {
                    *node.right = Option::Some(next_node);
                    return Option::None;
                }
                _ => return Option::None,
            }
        } else {
            let resp = insert_helper(&mut node.left, url, content, title, headings, highlighted);
            match resp {
                Some(next_node) => {
                    *node.left = Option::Some(next_node);
                    return Option::None;
                }
                _ => return Option::None,
            }
        }
    }

    pub fn insert(url: &str, content: &str, title: &str, headings: &str, highlighted: &str) {
        println!("url_index insert triggered => url : {url}");
        let mut root_ref = root.write().unwrap();
        if root_ref.is_none() {
            let hash = get_hash(content);
            *root_ref = Some(Node {
                url: String::from(url),
                hash: hash,
                content: String::from(content),
                title: String::from(title),
                headings: String::from(headings),
                highlighted: String::from(highlighted),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
                timestamp: chrono::Utc::now(),
            });
            println!("root is updated");
            handle_index_config_update(url, content, title, headings, highlighted);
            return;
        }
        insert_helper(&mut root_ref, url, content, title, headings, highlighted);
        handle_index_config_update(url, content, title, headings, highlighted);
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
