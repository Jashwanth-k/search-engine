use dotenv;
use std::{error::Error, thread};
use tokio;

mod crawler;
mod inverted_index;
mod url_index;

async fn init() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    let handle = thread::spawn(|| {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            crawler::main::init().await;
        });
    });
    inverted_index::main::insert("abc", "https://microsoft.com");
    inverted_index::main::insert("jlsd", "https://microsoft.com");
    inverted_index::main::insert("sdk", "https://google.com");
    inverted_index::main::insert("sdk", "https://gemini.google.com");
    inverted_index::main::insert("jlsd", "https://teams.microsoft.com");
    let result = inverted_index::main::get_by_text("sdk");
    println!("index result : {:?}", result);
    let result = inverted_index::main::get_by_text("jlsd");
    println!("index result : {:?}", result);
    // tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    
    // let str1 = String::from("abc");
    // let str2 = "baa";
    // if *str1 >= *str2 {
    //     println!("{str1} is greater");
    // }
    handle.join().unwrap();
    Ok(())
}

#[tokio::main]
async fn main() {
    let result = init().await;
    if result.is_err() {
        println!("error in main init : {:?}", result);
    }
}
