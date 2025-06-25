use axum::{
    Router,
    extract::{Json, Path},
    routing,
};
use dotenv;
use serde::{Deserialize, Serialize};
use std::{error::Error, thread};
use tokio;

mod crawler;
mod inverted_index;
mod url_index;

#[derive(Serialize, Deserialize)]
struct SearchPagesResult {
    url: String,
    meta_content: String,
}

#[derive(Serialize, Deserialize)]
struct ApiResp {
    msg: String,
    data: Vec<SearchPagesResult>,
}

#[axum::debug_handler]
async fn get_pages_by_search_text(Path(search_text): Path<String>) -> Json<ApiResp> {
    let url_resp = inverted_index::main::get_by_text(&search_text);
    match url_resp {
        None => {
            let data = ApiResp {
                msg: "No Pages Found!".to_string(),
                data: vec![],
            };
            return Json(data);
        }
        Some(urls) => {
            let data = urls
                .iter()
                .map(|url| {
                    let url_data = url_index::main::get_by_url(url);
                    if url_data.is_none() {
                        return SearchPagesResult {
                            url: url.to_string(),
                            meta_content: String::new(),
                        };
                    }
                    let url_data = url_data.unwrap();
                    return SearchPagesResult {
                        url: url.to_string(),
                        meta_content: url_data.meta_content,
                    };
                })
                .collect();
            return Json(ApiResp {
                msg: "Data Fetched successfully".to_string(),
                data: data,
            });
        }
    }
}

async fn init() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    let handle = thread::spawn(|| async {
        let app = Router::new().route("/search/{search_text}", routing::get(get_pages_by_search_text));
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });
    let _ = thread::spawn(|| {
        let _ = inverted_index::main::index();
        let _ = url_index::main::index();
        // let runtime = tokio::runtime::Runtime::new().unwrap();
        // runtime.block_on(async {
        //    crawler::main::init().await;
        // });
    });
    // tokio::task::spawn(crawler::main::init().await);
    let data = handle.join().unwrap();
    Ok(())
}

#[tokio::main]
async fn main() {
    let result = init().await;
    if result.is_err() {
        println!("error in main init : {:?}", result);
    }
}
