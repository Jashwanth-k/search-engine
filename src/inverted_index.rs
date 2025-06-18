use lazy_static::lazy_static;
use std::sync::{Arc, RwLock};

pub struct Node {
    text: String,
    urls: Vec<String>,
    left: Box<Option<Node>>,
    right: Box<Option<Node>>,
}

lazy_static! {
    static ref root: Arc<RwLock<Option<Node>>> = Arc::new(RwLock::new(Option::None));
}

pub mod main {  
    use super::*;
    pub fn index() {}

    fn insert_helper(node: &mut Option<Node>, text: &str, url: &str) -> Option<Node> {
        if node.is_none() {
            let new_node = Some(Node {
                text: String::from(text),
                urls: Vec::from([String::from(url)]),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
            });
            return new_node;
        }
        let node = node.as_mut().unwrap();
        if node.text == text {
            node.urls.push(String::from(url));
            return Option::None
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

    pub fn insert(text: &str, url: &str) {
        println!("inverted_index insert triggered => text : {text}, url : {url}");
        let mut root_ref = root.write().unwrap();
        if root_ref.is_none() {
            *root_ref = Some(Node {
                text: String::from(text),
                urls: Vec::from([String::from(url)]),
                left: Box::new(Option::None),
                right: Box::new(Option::None),
            });
            println!("root is updated");
            return;
        }
        insert_helper( &mut root_ref, text, url);
    }

    fn get_helper(node: &Option<Node>, text: &str) -> Option<Vec<String>> {
        if node.is_none() {
            return Option::None;
        }
        let node = node.as_ref().unwrap();
        if node.text == text {
            return Option::Some(node.urls.clone());
        } else if *node.text >= *text {
            return get_helper(&node.right, text);
        } else {
            return get_helper(&node.left, text);
        }
    }

    pub fn get_by_text(text: &str) -> Option<Vec<String>> {
        let root_ref = root.read().unwrap();
        return get_helper(&root_ref, text);
    }
}
