use lazy_static::lazy_static;
use std::sync::{Arc, RwLock};
use md5;

#[derive(Clone)]
pub struct Node {
    url: String,
    pub hash: String,
    meta_content: String,
    left: Box<Option<Node>>,
    right: Box<Option<Node>>,
}

lazy_static! {
    static ref root: Arc<RwLock<Option<Node>>> = Arc::new(RwLock::new(Option::None));
}

pub mod main {  
    use super::*;
    pub fn index() {}

    pub fn get_hash(content: &str) -> String {
        let hash = md5::compute(content).iter().map(|x| x.to_string()).collect();
        return hash;
    }
    
    // fn re_index() {

    // }

    fn insert_helper(node: &mut Option<Node>, url: &str, content: &str, meta_content: &str) -> Option<Node> {
        if node.is_none() {
            let hash = get_hash(content);
            let new_node = Some(Node {
                url: String::from(url),
                hash: hash,
                meta_content: String::from(meta_content),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
            });
            return new_node;
        }
        let node = node.as_mut().unwrap();
        if node.url == url {
            node.meta_content = String::from(meta_content);
            node.hash = get_hash(content);
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

    pub fn insert(url: &str, content: &str, meta_content: &str) {
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
            });
            println!("root is updated");
            return;
        }
        insert_helper( &mut root_ref, url, content, meta_content);
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
