use crate::url_index;
use chrono;
use lazy_static::lazy_static;
use reqwest::Client;
use scraper::Html;
use scraper::Selector;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::env;
use std::error::Error;
use std::fs;
use std::{cmp, thread, time::Duration};

#[derive(Debug)]
struct UrlResp {
    urls: Vec<String>,
    is_fetched: bool,
}

struct QueueEle {
    urls: Vec<String>,
    depth: u8,
}

lazy_static! {
    static ref client: Client = Client::new();
}

use std::io::Write;
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

    fn save_fetch_log(url: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let filepath = "data/fetch_log.txt";
        let mut file_data = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(filepath)?;
        let write_content = format!("{}\n", url);
        let _ = file_data.write(write_content.as_bytes());
        Ok(())
    }

    async fn fetch_data(url: &str) -> Result<String, Box<dyn Error + Sync + Send>> {
        println!("started fetching url : {url}");
        let data = client
            .get(url)
            .timeout(Duration::from_secs(10))
            .header("accept", "text/html")
            .header("user-agent", "crawler")
            .send()
            .await?
            .text()
            .await?;
        let _ = save_fetch_log(url);
        Ok(data)
    }

    fn get_meta_description(document: &Html) -> Result<String, Box<dyn Error + Send + Sync>> {
        let meta_description = document
            .select(&Selector::parse("meta[name='description']").unwrap())
            .next()
            .and_then(|element| element.value().attr("content"))
            .unwrap_or("");
        Ok(meta_description.into())
    }

    fn get_urls(document: &Html) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
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

    fn get_by_selectors(
        document: &Html,
        selector: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        // ("body, title, h1, h2, h3, h4, h5, h6, p, strong, b, i, em")
        let body_selector = Selector::parse(selector).unwrap();
        // let body_element = document.select(&body_selector).next();
        // if body_element.is_none() {
        //     return Ok("".to_string());
        // }
        // let body_element = body_element.unwrap();
        let mut text_parts = Vec::new();
        for element in document.select(&body_selector) {
            let text = element.text().collect::<Vec<_>>().join("");
            if !text.trim().is_empty() {
                text_parts.push(text);
            }
        }
        let content = text_parts
            .join(" ")
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
            .to_lowercase();
        // let meta_description = get_meta_description(&document)?;
        // content.push_str(&(String::from(" ") + &meta_description));
        // println!("selector: {selector}, content: {content}");
        Ok(content)
    }

    async fn handle_url(
        url: &str,
        force_fetch: bool,
    ) -> Result<UrlResp, Box<dyn Error + Send + Sync>> {
        if !url.contains("http://") && !url.contains("https://") {
            return Ok(UrlResp {
                urls: vec![],
                is_fetched: false,
            });
        }
        let data = fetch_data(&url).await?;
        let document = scraper::Html::parse_document(&data);
        let urls = get_urls(&document)?;
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
                    urls: urls,
                    is_fetched: true,
                });
            }
        }
        let title = get_by_selectors(&document, "title")?;
        let headings = get_by_selectors(&document, "h1, h2, h3, h4, h5, h6")?;
        let highlighted = get_by_selectors(
            &document,
            "strong, b, i, em, li, [class*='highlight'], [class*='important'], [class*='bold'], [class*='italic'], [class*='emphasize']",
        )?;
        let content = get_by_selectors(
            &document,
            "div, article, main, section, p, [class*='content'], [class*='post'], [class*='story']",
        )?;
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
            url_index::main::insert(url, &content, &title, &headings, &highlighted);
            crate::inverted_index::main::insert_by_content(
                url,
                &content,
                &title,
                &headings,
                &highlighted,
            );
        }
        Ok(UrlResp {
            urls: urls,
            is_fetched: true,
        })
    }

    async fn handle_urls(
        urls: Vec<String>,
        depth: u8,
        force_fetch: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut dqueue: VecDeque<QueueEle> = VecDeque::from([QueueEle { urls, depth }]);
        let mut visited: HashSet<String> = HashSet::new();
        while dqueue.len() != 0 {
            let QueueEle {
                urls: curr_urls,
                depth: curr_depth,
            } = dqueue.pop_front().unwrap();
            if curr_depth == 0 {
                continue;
            }
            let mut join_handles = Vec::new();
            for url in curr_urls {
                if visited.contains(&url) {
                    continue;
                }
                visited.insert(url.clone());
                let handle_res = tokio::spawn(async move { handle_url(&url, force_fetch).await });
                join_handles.push(handle_res);
            }
            let mut next_results: Vec<Vec<String>> = Vec::new();
            for handle in join_handles {
                let handled_resp = handle.await;
                if handled_resp.is_err() {
                    println!("error in awaiting handling url {:?}", handled_resp);
                    continue;
                }
                let handled_resp = handled_resp.unwrap();
                if handled_resp.is_err() {
                    println!("error in handling url {:?}", handled_resp);
                    continue;
                }
                let UrlResp {
                    urls,
                    is_fetched: _,
                } = handled_resp.unwrap();
                let mut idx = 0;
                for url in urls {
                    while next_results.len() <= idx {
                        next_results.push(Vec::new());
                    }
                    next_results[idx].push(url);
                    idx += 1;
                }
            }
            for urls in next_results {
                dqueue.push_back(QueueEle {
                    urls,
                    depth: curr_depth - 1,
                });
            }
        }
        Ok(())
    }

    pub async fn init(
        seed_urls: Vec<String>,
        crawl_depth: u8,
    ) -> Result<(), Box<dyn Error + Send>> {
        println!("seed urls init ==> {:?}", seed_urls);
        let handle_resp = handle_urls(seed_urls, crawl_depth, false).await;
        if handle_resp.is_err() {
            println!("error in handling seed urls : {:?}", handle_resp);
        }
        Ok(())
    }

    pub fn init_multiple() -> Result<(), Box<dyn Error + Send + Sync>> {
        let seed_urls = get_seed_file();
        let mut available_threads = thread::available_parallelism().unwrap().get();
        let crawl_threads_multiplier = &env::var("CRAWL_THREADS_MULTIPLIER")
            .unwrap_or(String::from("2"))
            .parse::<u8>()
            .unwrap();
        available_threads = cmp::max(available_threads * *crawl_threads_multiplier as usize, 1);
        println!("available crawler threads : {available_threads}");
        if seed_urls.is_err() {
            panic!("error while getting seed urls : {:?}", seed_urls);
        }
        let seed_urls = seed_urls.unwrap_or(vec![]);
        let mut splitted_seed_urls: Vec<Vec<String>> =
            (0..available_threads).map(|el| Vec::new()).collect();
        for (idx, url) in seed_urls.iter().enumerate() {
            splitted_seed_urls[idx % available_threads as usize].push(url.to_string());
        }
        println!(
            "splitted urls => {:?}, total => {}",
            splitted_seed_urls,
            seed_urls.len()
        );
        let mut threads = Vec::new();
        for seed_urls in splitted_seed_urls {
            let handle = thread::spawn(|| {
                let crawl_depth = &env::var("CRAWL_DEPTH")
                    .unwrap_or(String::from("10"))
                    .parse::<u8>()
                    .unwrap();
                let runtime = tokio::runtime::Runtime::new().unwrap();
                runtime.block_on(async {
                    let handle_resp = init(seed_urls, *crawl_depth).await;
                    if handle_resp.is_err() {
                        println!("error in handling seed urls : {:?}", handle_resp);
                    }
                })
            });
            threads.push(handle);
        }
        for handle in threads {
            let _ = handle.join().unwrap();
        }
        Ok(())
    }

    pub async fn handle_url_req(url: String) -> () {
        let crawl_depth = &env::var("CRAWL_DEPTH")
            .unwrap_or(String::from("10"))
            .parse::<u8>()
            .unwrap();
        handle_urls(Vec::from([url.to_string()]), *crawl_depth, true)
            .await
            .unwrap();
        println!("url processed resp url: {url}");
    }
}
