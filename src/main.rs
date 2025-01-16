use cached::proc_macro::cached;
use std::collections::HashMap;
use std::fs;
use once_cell::sync::Lazy;
use tokio::sync::OnceCell;
use serde_json::Value;
use sqlx::{mysql::MySqlPoolOptions, MySql, Pool};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::prelude::*;
use tracing::{error, info, debug, warn};

static LOGIN_ID: Lazy<HashMap<String, String>> = Lazy::new(|| {
    serde_json::from_str::<Value>(include_str!("../login_info.json")).unwrap().as_object().unwrap().iter().map(|(k, v)| {
        (k.clone(), v.clone().as_str().unwrap().to_string())
    }).collect()
});
static DATABASE_POOL: OnceCell<Pool<MySql>> = OnceCell::const_new();
static PARTNER_ID: i32 = 13;
#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer()).init();
    println!(env!("DATABASE_URL"));
    DATABASE_POOL.set(MySqlPoolOptions::new().connect(env!("DATABASE_URL")).await.unwrap()).unwrap();
    for query in include_str!("../init_db.sql").strip_suffix(";").unwrap().split(';') {
        sqlx::query(&format!("{query};")).execute(DATABASE_POOL.get().unwrap()).await.unwrap();
        info!("{query};");
    }
    info!("login id: {:?}",*LOGIN_ID);
    update_orical_user().await;
}
#[tracing::instrument]
async fn generate_client(authorized: bool) -> reqwest::Client {
    #[cached(time = 900)]
    async fn generate_secure_token() -> String {
        let client = reqwest::Client::new();
        let custom_token = client.post("https://account-api.orical.jp/firebase_user/generate_custom_token?idprovider_key=helloproject_id").
            json(&*LOGIN_ID).send().await.unwrap().json::<Value>().await.unwrap()["custom_token"].as_str().unwrap().to_string();
        client.post("https://identitytoolkit.googleapis.com/v1/accounts:signInWithCustomToken?key=AIzaSyABJQ_1lLpugYT2kuzdCsRmcx0P8QRG16s").
            json(&HashMap::from([("token", custom_token), ("returnSecureToken", "true".to_string())])).send().await.unwrap().json::<Value>().await.unwrap()["idToken"].as_str().unwrap().to_string()
    }
    if !authorized {
        reqwest::Client::new()
    } else {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, reqwest::header::HeaderValue::from_str(&*generate_secure_token().await).unwrap());
        headers.get_mut(reqwest::header::AUTHORIZATION).unwrap().set_sensitive(true);
        reqwest::Client::builder().default_headers(headers).build().unwrap()
    }
}
#[tracing::instrument]
async fn update_orical_user() {
    let client = generate_client(true).await;
    let all_users_count = client.get(format!("https://api-helloproject.orical.jp/partners/{PARTNER_ID}/ranking/top100?page=1&per=1")).send().await.unwrap().json::<Value>().await.unwrap()["my_rank"]["num_rivals"].as_i64().unwrap();

}