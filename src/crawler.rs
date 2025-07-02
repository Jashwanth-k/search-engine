use crate::url_index;
use chrono;
use reqwest::Client;
use scraper::Html;
use scraper::Selector;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs;

#[derive(Debug)]
struct UrlResp {
    urls: Vec<String>,
    is_fetched: bool,
}

pub mod main {
    use super::*;
    fn get_seed_file() -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let filepath = &env::var("SEED_URLS_FILE_PATH")?;
        let file_data = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(filepath)?;
        let file_data = fs::read_to_string(filepath)?;
        let seed_urls = file_data.lines().map(String::from).collect();
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

    async fn handle_url(
        url: &str,
        client: &Client,
        force_fetch: bool,
    ) -> Result<UrlResp, Box<dyn Error + Send>> {
        if !url.contains("http://") && !url.contains("https://") {
            return Ok(UrlResp {
                urls: vec![],
                is_fetched: false,
            });
        }
        let url_node = url_index::main::get_by_url(url);
        if url_node.is_some() && force_fetch == false {
            let url_timestamp = url_node.as_ref().unwrap().timestamp;
            let curr_timestamp = chrono::Utc::now();
            let date_diff_days = (curr_timestamp - url_timestamp).num_days();
            let req_date_diff = &env::var("CRAWL_DATE_DIFF_FOR_UPDATE")
                .unwrap_or(String::from("3"))
                .parse::<i64>()
                .unwrap();
            if date_diff_days < *req_date_diff {
                return Ok(UrlResp {
                    urls: vec![],
                    is_fetched: false,
                });
            }
        }
        println!("started fetching url : {url}");
        let data = fetch_data(&url, &client).await.map_err(Into::into)?;
        let document = scraper::Html::parse_document(&data);
        let urls = get_urls(&document).unwrap();
        let content = get_content(&document).unwrap();
        let meta_description: String = get_meta_description(&document).unwrap();
        println!("body parsed for url : {url}");
        let mut index_content = true;
        match url_node {
            Some(node) => {
                let content_hash = node.hash;
                let curr_hash = url_index::main::get_hash(&content);
                if curr_hash == content_hash {
                    index_content = false;
                }
            }
            None => (),
        }
        if index_content {
            url_index::main::insert(url, &content, &meta_description);
            crate::inverted_index::main::insert_by_content(url, &content);
        }
        Ok(UrlResp {
            urls: urls,
            is_fetched: true,
        })
    }

    async fn handle_urls(
        urls: Vec<String>,
        client: &Client,
        depth: u8,
        force_fetch: bool,
    ) -> Result<bool, Box<dyn Error + Send>> {
        if depth == 0 {
            return Ok(false);
        }
        let is_fetched = false;
        for url in urls {
            let handled_resp = handle_url(&url, &client, force_fetch).await;
            if handled_resp.is_err() {
                println!("error in handling url : {url}, error : {:?}", handled_resp);
                continue;
            }
            let UrlResp { urls, is_fetched } = handled_resp.unwrap();
            let next_res = Box::pin(handle_urls(urls, client, depth - 1, force_fetch)).await;
            match next_res {
                Ok(next_fetched) => is_fetched || next_fetched,
                Err(_) => false,
            };
        }
        Ok(is_fetched)
    }

    pub async fn init() -> Result<(), Box<dyn Error + Send>> {
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
        let handle_resp = handle_urls(seed_urls, &client, *crawl_depth, false).await;
        if handle_resp.is_err() {
            println!("error in handling seed urls : {:?}", handle_resp);
        }
        Ok(())
    }

    pub async fn handle_url_req(url: String) -> Result<(), Box<dyn Error + Send + Sync>> {
        let crawl_depth = &env::var("CRAWL_DEPTH")
            .unwrap_or(String::from("10"))
            .parse::<u8>()
            .unwrap();
        let client = Client::new();
        let pages_processed =
            handle_urls(Vec::from([url.to_string()]), &client, *crawl_depth, true)
                .await
                .unwrap();
        println!("url processed resp url: {url}, status: {pages_processed}");
        Ok(())
    }
}
