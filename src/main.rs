use axum::{
    Router,
    extract::{Json, Path},
    response::Html,
    routing,
};
use dotenv;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{env, error::Error, fs, thread};
use tokio;

mod crawler;
mod inverted_index;
mod url_index;

#[derive(Serialize, Deserialize)]
struct ApiRespSearch {
    msg: String,
    data: Vec<inverted_index::ResultScore>,
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
    let url_resp = inverted_index::main::get_text_by_scoring(&search_text);
    if let Err(err) = url_resp {
        println!("search text error => text: {search_text}, error: {:?}", err);
        let data = ApiRespSearch {
            msg: "No Pages Found!".to_string(),
            data: vec![],
        };
        return Json(data);
    }
    let url_resp = url_resp.unwrap();
    println!(
        "search text resp => text: {search_text}, result: {:?}",
        url_resp
    );
    return Json(ApiRespSearch {
        msg: "Data Fetched successfully".to_string(),
        data: url_resp,
    });
}

fn init() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    let tcp_thread = thread::spawn(|| {
        let app = Router::new()
            .route(
                "/api/search/{search_text}",
                routing::get(get_pages_by_search_text),
            )
            .route("/api/index", routing::post(crawl_index_url))
            .route("/", routing::get(get_homepage));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
            axum::serve(listener, app).await.unwrap()
        });
    });
    let inverted_index_thread = thread::spawn(|| {
        let _ = inverted_index::main::index();
    });
    let url_index_thread = thread::spawn(|| {
        let _ = url_index::main::index();
    });
    let _ = inverted_index_thread.join().unwrap();
    let _ = url_index_thread.join().unwrap();
    let index_save_interval = env::var("INDEX_SAVE_INTERVAL_MIN")
        .unwrap_or(String::from("30"))
        .parse::<u8>()
        .unwrap();
    let _ = thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(index_save_interval as u64 * 60));
            let _ = url_index::main::write_to_file();
        }
    });
    let stop_crawler = env::var("STOP_CRAWLER")
        .unwrap_or("false".to_string())
        .parse::<bool>()
        .unwrap();
    if stop_crawler != true {
        thread::spawn(|| {
            loop {
                let _ = crawler::main::init_multiple();
                thread::sleep(Duration::from_secs(60 * 10));
            }
        });
    }
    let _ = tcp_thread.join();
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
    let api_base_url = env::var("API_BASE_URL").unwrap_or("http://localhost:8080".to_string());
    let file_data = file_data.replace("__API_BASE_URL__", &api_base_url);
    Html(file_data)
}
