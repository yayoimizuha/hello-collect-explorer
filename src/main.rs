use cached::proc_macro::cached;
use std::collections::HashMap;
use std::fs;
use chrono::DateTime;
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
        debug!("{query};");
    }
    debug!("login id: {:?}",*LOGIN_ID);
    // update_orical_user().await;
    update_cardpacks().await;
    update_cards().await;
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
    let chunk_size = 100;
    let client = generate_client(true).await;
    let all_users_count = client.get(format!("https://api-helloproject.orical.jp/partners/{PARTNER_ID}/ranking/top100?page=1&per=1")).send().await.unwrap().json::<Value>().await.unwrap()["my_rank"]["num_rivals"].as_i64().unwrap();
    debug!("all_users_count: {all_users_count}");
    // let all_users_count = 5000_i64;
    let mut begin = DATABASE_POOL.get().unwrap().begin().await.unwrap();
    sqlx::query("TRUNCATE TABLE orical_user").execute(&mut *begin).await.unwrap();
    for i in 1..=(all_users_count as f64 / chunk_size as f64).ceil() as i32 {
        for person_data in client.get(format!("https://api-helloproject.orical.jp/partners/{PARTNER_ID}/ranking/top100?page={i}&per={chunk_size}")).send().await.unwrap().json::<Value>().await.unwrap()["rankings"].as_array().unwrap() {
            // debug!("Rank: {person_data}");
            let screen_name = person_data["partner_user"]["screen_name"].as_str().unwrap();
            let user_id = person_data["partner_user"]["user_id"].as_i64().unwrap();
            let orical_id = person_data["partner_user"]["orica"]["id"].as_i64().unwrap();
            let comment = person_data["partner_user"]["orica"]["comment"].as_str().unwrap_or_else(|| { "" });
            let frontal_card_ids = match person_data["partner_user"]["orica"].get("card_ids") {
                Some(x) => { x.as_array().unwrap().iter().map(|v| format!("{}", v.as_i64().unwrap())).collect::<Vec<_>>().join(",") }
                None => { "".to_string() }
            };
            debug!("screen_name: {screen_name}");
            debug!("\tuser_id: {user_id}");
            debug!("\torical_id: {orical_id}");
            debug!("\tcomment: {comment}");
            debug!("\tfrontal_card_ids: {frontal_card_ids}");
            sqlx::query("INSERT INTO orical_user(user_id,orical_id,screen_name,comment,frontal_card_ids) VALUES (?, ?, ?, ?, ?) ON DUPLICATE KEY UPDATE user_id = ?;")
                .bind(user_id).bind(orical_id).bind(screen_name).bind(comment).bind(frontal_card_ids).bind(user_id).execute(&mut *begin).await.unwrap();
        }
    }
    begin.commit().await.unwrap();
}

#[tracing::instrument]
async fn update_cardpacks() {
    let chunk_size = 25;
    let client = generate_client(true).await;

    let mut begin = DATABASE_POOL.get().unwrap().begin().await.unwrap();
    sqlx::query("TRUNCATE TABLE cardpacks").execute(&mut *begin).await.unwrap();

    let mut page = 1;
    loop {
        let query = format!("https://api-helloproject.orical.jp/cardpacks?partner_id={PARTNER_ID}&page={page}&per={chunk_size}&return_closed=true&order=available_at");
        let resp = client.get(query).send().await.unwrap().json::<Value>().await.unwrap();
        let cardpack_array = resp.as_array().unwrap();
        for cardpack in cardpack_array {
            let name = cardpack["name"].as_str().unwrap();
            let description = cardpack["description"].as_str().unwrap();
            let id = cardpack["id"].as_i64().unwrap();
            let available_at = DateTime::parse_from_rfc3339(cardpack["available_at"].as_str().unwrap()).unwrap();
            let closes_at = DateTime::parse_from_rfc3339(cardpack["closes_at"].as_str().unwrap()).unwrap();

            sqlx::query("INSERT INTO cardpacks(cardpack_id,name,description,available_at,closes_at) VALUES (?, ?, ?, ?, ?) ON DUPLICATE KEY UPDATE cardpack_id = ?;")
                .bind(id).bind(name).bind(description).bind(available_at.naive_local()).bind(closes_at.naive_local()).bind(id).execute(&mut *begin).await.unwrap();


            debug!("name: {}", name);
            debug!("\tdescription: {}", description);
            debug!("\tid: {}", id);
            debug!("\tavailable_at: {}", available_at);
            debug!("\tcloses_at: {}", closes_at);
        }
        page += 1;
        if cardpack_array.len() != chunk_size { break; }
    }
    begin.commit().await.unwrap();
}

#[tracing::instrument]
async fn update_cards() {
    let cardpack_ids = sqlx::query_as::<_, (i64,)>("SELECT cardpack_id FROM cardpacks;")
        .fetch_all(DATABASE_POOL.get().unwrap()).await.unwrap().into_iter().map(|v| { v.0 }).collect::<Vec<_>>();

    let chunk_size = 25;
    let client = generate_client(false).await;

    let mut begin = DATABASE_POOL.get().unwrap().begin().await.unwrap();
    sqlx::query("TRUNCATE TABLE cards;").execute(&mut *begin).await.unwrap();

    for cardpack_id in cardpack_ids {
        let mut page = 1;
        loop {
            let query = format!("https://api-helloproject.orical.jp/cards/index_by_cardpacks?partner_id={PARTNER_ID}&cardpack_id={cardpack_id}&page={page}&per={chunk_size}");
            let resp = client.get(query).send().await.unwrap().json::<Value>().await.unwrap();
            let card_array = resp.as_array().unwrap();
            for card in card_array {
                debug!("card id: {}", card["id"].as_i64().unwrap());
                debug!("\tcardpack_id: {}", card["cardpacks"].as_array().unwrap().iter().next().unwrap()["id"].as_i64().unwrap());
                debug!("\trarity: {}", card["rarity"].as_i64().unwrap());
                debug!("\tcard_type: {}",match card["person_id"].as_i64(){
                    Some(_) => {"person"},
                    None => {"unit"}
                });
                debug!("\tcharacter_id: {}",match card["person_id"].as_i64(){
                    Some(x) => {x},
                    None => {card["unit_id"].as_i64().unwrap()}
                });
                debug!("\tseason_id: {}", card["season_id"].as_i64().unwrap());
                debug!("\tfrontimage: {}", card["frontimage"].as_str().unwrap());
                debug!("\tfrontimage_thumbnail: {}", card["frontimage_thumbnail"].as_str().unwrap());
            }
            page += 1;
            if card_array.len() != chunk_size { break; }
        }
    }
}