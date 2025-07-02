use axum::{
    Router,
    extract::{Json, Path},
    response::Html,
    routing,
};
use dotenv;
use serde::{Deserialize, Serialize};
use std::{collections::BinaryHeap, time::Duration};
use std::{env, error::Error, fs, thread};
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
struct ApiRespSearch {
    msg: String,
    data: Vec<SearchPagesResult>,
}

#[derive(Serialize, Deserialize)]
struct ApiRespIndex {
    msg: String,
}

#[derive(Serialize, Deserialize)]
struct IndexPayload {
    url: String,
}

#[axum::debug_handler]
async fn crawl_index_url(Json(payload): Json<IndexPayload>) -> Json<ApiRespIndex> {
    let url = payload.url;
    tokio::spawn(async { crawler::main::handle_url_req(url).await });
    Json(ApiRespIndex {
        msg: "pages indexing finished".to_string(),
    })
}
#[axum::debug_handler]
async fn get_pages_by_search_text(Path(search_text): Path<String>) -> Json<ApiRespSearch> {
    let top_k = 10;
    let url_resp = inverted_index::main::get_by_text(&search_text);
    let mut heap = BinaryHeap::<(i64, String)>::new();
    if let None = url_resp {
        let data = ApiRespSearch {
            msg: "No Pages Found!".to_string(),
            data: vec![],
        };
        return Json(data);
    }
    let urls_map = url_resp.unwrap();
    for (word, count) in urls_map {
        let count = -(count as i64);
        heap.push((count, word));
        if heap.capacity() > top_k {
            heap.pop();
        }
    }

    println!(
        "search text resp => text: {search_text}, result: {:?}",
        heap
    );
    let urls = heap
        .into_sorted_vec()
        .into_iter()
        .map(|(count, url)| url)
        .collect::<Vec<String>>();
    // println!("fetched urls {:?}", urls);
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
    return Json(ApiRespSearch {
        msg: "Data Fetched successfully".to_string(),
        data: data,
    });
}

#[tokio::main]
async fn init() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    let index_save_interval = env::var("INDEX_SAVE_INTERVAL_MIN")
        .unwrap_or(String::from("30"))
        .parse::<u8>()
        .unwrap();
    thread::spawn(move || {
        let inverted_index_thread = thread::spawn(|| {
            let _ = inverted_index::main::index();
        });
        let url_index_thread = thread::spawn(|| {
            let _ = url_index::main::index();
        });
        let _ = inverted_index_thread.join().unwrap();
        let _ = url_index_thread.join().unwrap();
        let _ = thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(index_save_interval as u64 * 30));
                let _ = url_index::main::write_to_file();
            }
        });
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            loop {
                let _ = crawler::main::init().await;
            }
        });
    });
    let app = Router::new()
        .route(
            "/search/{search_text}",
            routing::get(get_pages_by_search_text),
        )
        .route("/index", routing::post(crawl_index_url))
        .route("/home", routing::get(get_homepage));
    let tcp_thread = thread::spawn(|| async {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
        axum::serve(listener, app).await.unwrap()
    });
    let _ = tcp_thread.join().unwrap().await;
    Ok(())
}

fn main() {
    let result = init();
    if result.is_err() {
        println!("error in main init : {:?}", result);
    }
}

// Homepage
#[axum::debug_handler]
async fn get_homepage() -> Html<String> {
    let error_page = r#"
    <!DOCTYPE html>
    <html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>Error</title>
        <style>
            html, body {
                height: 100%;
                margin: 0;
                font-family: system-ui, sans-serif;
                background: #f8f9fa;
            }
            body {
                display: flex;
                align-items: center;
                justify-content: center;
                text-align: center;
                color: #555;
            }
            div {
                line-height: 1.5;
            }
            h1 {
                font-size: 5rem;
                font-weight: 300;
                margin: 0;
            }
        </style>
    </head>
    <body>
        <div>
            <h1>:(</h1>
            <p>Oops! Something went wrong.</p>
        </div>
    </body>
    </html>
    "#;
    let filepath = &env::var("HOMEPAGE_FILE_PATH");
    if filepath.is_err() {
        return Html(error_page.to_string());
    }
    let filepath = filepath.as_ref().unwrap();
    let file_data = fs::read_to_string(filepath);
    if file_data.is_err() {
        return Html(error_page.to_string());
    }
    let file_data = file_data.unwrap();
    Html(file_data.to_string())
}
