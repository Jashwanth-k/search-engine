use crate::url_index;
use reqwest::Client;
use scraper::Html;
use scraper::{ElementRef, Selector};
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs;

pub mod main {
    use super::*;
    fn get_seed_file() -> Result<Vec<String>, Box<dyn Error>> {
        let filepath = &env::var("SEED_URLS_FILE_PATH")?;
        let filedata = fs::read_to_string(filepath)?;
        let seed_urls = filedata.lines().map(String::from).collect();
        return Ok(seed_urls);
    }

    async fn fetch_data(url: &str, client: &Client) -> Result<String, Box<dyn Error>> {
        let data = client
            .get(url)
            .header("accept", "text/html")
            .header("user-agent", "crawler")
            .send()
            .await?
            .text()
            .await?;
        Ok(data)
    }

    fn get_meta_description(document: &Html) -> Result<String, Box<dyn Error>> {
        let meta_description = document
            .select(&Selector::parse("meta[name='description']").unwrap())
            .next()
            .and_then(|element| element.value().attr("content"))
            .unwrap_or("");
        Ok(meta_description.into())
    }

    fn get_urls(document: &Html) -> Result<Vec<String>, Box<dyn Error>> {
        let mut urls: HashSet<String> = HashSet::new();
        let url_selector = Selector::parse("a").unwrap();
        for element in document.select(&url_selector) {
            if let Some(href) = element.value().attr("href") {
                urls.insert(href.into());
            }
        }
        let urls = urls.into_iter().collect();
        Ok(urls)
    }

    fn get_content(document: &Html) -> Result<String, Box<dyn Error>> {
        let body_selector = Selector::parse("body, h1, h2, h3, h4, h5, h6, p, li, strong, em, label, input[type='text'], textarea, [aria-label]")
            .unwrap();
        let body_element = document.select(&body_selector).next().unwrap();

        let mut text_parts = Vec::new();
        for element in body_element.select(&body_selector) {
            text_parts.push(element.text().collect::<Vec<_>>().join(""));
        }
        let mut content = text_parts
            .join("")
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
            .to_lowercase();
        let meta_description = get_meta_description(&document)?;
        content.push_str(&(String::from(" ") + &meta_description));
        Ok(content)
    }

    async fn handle_url(url: &str, client: &Client) -> Result<Vec<String>, Box<dyn Error>> {
        if !url.contains("http://") && !url.contains("https://") {
            return Ok(vec![]);
        }
        println!("started fetching url : {url}");
        let data = fetch_data(&url, &client).await?;
        let document = scraper::Html::parse_document(&data);
        let urls = get_urls(&document)?;
        let content = get_content(&document)?;
        let meta_description: String = get_meta_description(&document)?;
        println!("body parsed for url : {url}");
        let url_node = url_index::main::get_by_url(url);
        let mut index_content = true;
        match url_node {
            Some(node) => {
                let content_hash = node.hash;
                let curr_hash = url_index::main::get_hash(&content);
                if curr_hash == content_hash {
                    index_content = false;
                }
            },
            None => (),
        }
        if index_content {
            url_index::main::insert(url, &content, &meta_description);
            for word in content.split_whitespace() {
                crate::inverted_index::main::insert(word, url);
            }
        }
        Ok(urls)
    }

    async fn handle_urls(
        urls: Vec<String>,
        client: &Client,
        depth: u8,
    ) -> Result<(), Box<dyn Error>> {
        if depth == 0 {
            return Ok(());
        }
        for url in urls {
            let handled_resp = handle_url(&url, &client).await;
            if handled_resp.is_err() {
                println!("error in handling url : {url}, error : {:?}", handled_resp);
                continue;
            }
            let handled_resp = handled_resp.unwrap();
            Box::pin(handle_urls(handled_resp, client, depth - 1)).await;
        }
        Ok(())
    }

    pub async fn init() {
        let seed_urls = get_seed_file();
        let crawl_depth = &env::var("CRAWL_DEPTH")
            .unwrap_or(String::from("10"))
            .parse::<u8>()
            .unwrap();
        if seed_urls.is_err() {
            panic!("error while getting seed urls : {:?}", seed_urls);
        }
        let seed_urls = seed_urls.unwrap_or(vec![]);
        let client = Client::new();
        let handle_resp = handle_urls(seed_urls, &client, *crawl_depth).await;
        if handle_resp.is_err() {
            panic!("error in handling seed urls : {:?}", handle_resp);
        }
    }
}
