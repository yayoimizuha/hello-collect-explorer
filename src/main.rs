use cached::proc_macro::cached;
use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use chrono::DateTime;
use futures::future;
use once_cell::sync::Lazy;
use tokio::sync::{OnceCell, Semaphore};
use serde_json::Value;
use sqlx::{mysql::MySqlPoolOptions, MySql, Pool};
use tokio::time::sleep;
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
    DATABASE_POOL.set(MySqlPoolOptions::new().max_connections(500).connect(env!("DATABASE_URL")).await.unwrap()).unwrap();
    for query in include_str!("../init_db.sql").strip_suffix(";").unwrap().split(';') {
        sqlx::query(&format!("{query};")).execute(DATABASE_POOL.get().unwrap()).await.unwrap();
        debug!("{query};");
    }
    debug!("login id: {:?}",*LOGIN_ID);
    update_orical_user().await;
    update_cardpacks().await;
    update_cards().await;

    let semaphore = Arc::new(Semaphore::new(3));
    let futures = list_users().await.into_iter().map(|id| {
        update_card_belong(id, semaphore.clone())
    }).collect::<Vec<_>>();
    future::join_all(futures).await;
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
    info!("start updating orical user...");
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
            let rank = person_data["rank"].as_i64().unwrap();
            let screen_name = person_data["partner_user"]["screen_name"].as_str().unwrap();
            let user_id = person_data["partner_user"]["user_id"].as_i64().unwrap();
            let orical_id = person_data["partner_user"]["orica"]["id"].as_i64().unwrap();
            let comment = person_data["partner_user"]["orica"]["comment"].as_str().unwrap_or_else(|| { "" });
            let frontal_card_ids = match person_data["partner_user"]["orica"].get("card_ids") {
                Some(x) => { x.as_array().unwrap().iter().map(|v| format!("{}", v.as_i64().unwrap())).collect::<Vec<_>>().join(",") }
                None => { "".to_string() }
            };
            debug!("screen_name: {screen_name}");
            debug!("\trank: {rank}");
            debug!("\tuser_id: {user_id}");
            debug!("\torical_id: {orical_id}");
            debug!("\tcomment: {comment}");
            debug!("\tfrontal_card_ids: {frontal_card_ids}");
            sqlx::query("REPLACE INTO orical_user(user_id,orical_id,screen_name,comment,frontal_card_ids) VALUES (?, ?, ?, ?, ?);")
                .bind(user_id).bind(orical_id).bind(screen_name).bind(comment).bind(frontal_card_ids).execute(&mut *begin).await.unwrap();
        }
    }
    begin.commit().await.unwrap();
    info!("end updating orical user...");
}

#[tracing::instrument]
async fn update_cardpacks() {
    info!("start updating orical cardpacks...");
    let chunk_size = 25;
    let client = generate_client(true).await;

    let mut begin = DATABASE_POOL.get().unwrap().begin().await.unwrap();
    // sqlx::query("TRUNCATE TABLE cardpacks").execute(&mut *begin).await.unwrap();

    let mut page = 1;
    loop {
        let query = format!("https://api-helloproject.orical.jp/cardpacks?partner_id={PARTNER_ID}&page={page}&per={chunk_size}&return_closed=true&order=available_at");
        let resp = client.get(&query).send().await.unwrap().json::<Value>().await.unwrap();
        debug!(query);
        let cardpack_array = resp.as_array().unwrap();
        for cardpack in cardpack_array {
            let name = cardpack["name"].as_str().unwrap();
            let description = cardpack["description"].as_str().unwrap();
            let cardpack_id = cardpack["id"].as_i64().unwrap();
            let available_at = DateTime::parse_from_rfc3339(cardpack["available_at"].as_str().unwrap()).unwrap();
            let closes_at = DateTime::parse_from_rfc3339(cardpack["closes_at"].as_str().unwrap()).unwrap();

            sqlx::query("REPLACE INTO cardpacks(cardpack_id,name,description,available_at,closes_at) VALUES(?, ?, ?, ?, ?);")
                .bind(cardpack_id).bind(name).bind(description).bind(available_at.naive_local()).bind(closes_at.naive_local()).execute(&mut *begin).await.unwrap();


            debug!("name: {}", name);
            debug!("\tdescription: {}", description);
            debug!("\tcardpack_id: {}", cardpack_id);
            debug!("\tavailable_at: {}", available_at);
            debug!("\tcloses_at: {}", closes_at);
            let mut page = 1;
            let chunk_size = 25;
            loop {
                let query = format!("https://api-helloproject.orical.jp/cards/index_by_cardpacks?partner_id=13&cardpack_id={cardpack_id}&page={page}&per=25");
                let resp = client.get(query).send().await.unwrap().json::<Value>().await.unwrap();
                let card_array = resp.as_array().unwrap();
                for card in card_array {
                    let card_id = card["id"].as_i64().unwrap();
                    sqlx::query("REPLACE INTO cardpack_belong(cardpack_id,card_id) VALUES(?, ?);")
                        .bind(cardpack_id).bind(card_id).execute(&mut *begin).await.unwrap();
                }
                page += 1;
                if card_array.len() != chunk_size { break; }
            }
        }
        page += 1;
        if cardpack_array.len() != chunk_size { break; }
    }
    begin.commit().await.unwrap();
    info!("end updating orical cardpacks...");
}

#[tracing::instrument]
async fn update_cards() {
    info!("start updating cards...");
    let chunk_size = 25;
    let client = generate_client(false).await;
    let mut begin = DATABASE_POOL.get().unwrap().begin().await.unwrap();

    for card_type in ["memorial", "non_memorial"] {
        for rarity in 1..=5 {
            let mut page = 1;
            loop {
                let query = format!("https://api-helloproject.orical.jp/cards?partner_id=13&card_type={card_type}&ownership_type=all&rarity={rarity}&page={page}&per={chunk_size}");
                let resp = client.get(query).send().await.unwrap().json::<Value>().await.unwrap();
                let card_array = resp.as_array().unwrap();
                for card in card_array {
                    let card_id = card["id"].as_i64().unwrap();
                    let memorial = card["memorial_id"].as_i64();
                    let rarity = card["rarity"].as_i64().unwrap();
                    let card_type = match card["person_id"].as_i64() {
                        Some(_) => { "person" }
                        None => { "unit" }
                    };
                    let character_id = match card["person_id"].as_i64() {
                        Some(x) => { x }
                        None => { card["unit_id"].as_i64().unwrap() }
                    };
                    let season_id = card["season_id"].as_i64().unwrap();
                    let frontimage = card["frontimage"].as_str().unwrap();
                    let frontimage_thumbnail = card["frontimage_thumbnail"].as_str().unwrap();

                    sqlx::query("REPLACE INTO cards(card_id,memorial,rarity,card_type,character_id,season_id,frontimage,frontimage_thumbnail) VALUES(?, ?, ?, ?, ?, ?, ?, ?);")
                        .bind(card_id).bind(memorial).bind(rarity).bind(card_type).bind(character_id).bind(season_id).bind(frontimage).bind(frontimage_thumbnail)
                        .execute(&mut *begin).await.unwrap();

                    debug!("card id: {}", card_id);
                    debug!("memorial id: {:?}", memorial);
                    debug!("\trarity: {}", rarity);
                    debug!("\tcard_type: {}",card_type);
                    debug!("\tcharacter_id: {}",character_id);
                    debug!("\tseason_id: {}", season_id);
                    debug!("\tfrontimage: {}", frontimage);
                    debug!("\tfrontimage_thumbnail: {}", frontimage_thumbnail);

                    match card_type {
                        "unit" => {
                            let unit_id = card["unit_id"].as_i64().unwrap();
                            let unit_name = card["unit"]["name"].as_str().unwrap();
                            let unit_image = card["unit"]["image"].as_str();
                            for member in card["unit"]["people"].as_array().unwrap() {
                                sqlx::query("REPLACE INTO characters(character_id,name,unit_member_id,image) VALUES(?, ?, ?, ?);")
                                    .bind(unit_id).bind(unit_name).bind(member["id"].as_i64().unwrap()).bind(unit_image)
                                    .execute(&mut *begin).await.unwrap();
                            }
                            debug!(unit_name);
                        }
                        "person" => {
                            let person_id = card["person_id"].as_i64().unwrap();
                            let person_name = card["person"]["name"].as_str().unwrap();
                            let person_image = card["person"]["profile_image"].as_str().unwrap();
                            sqlx::query("REPLACE INTO characters(character_id,name,unit_member_id,image) VALUES(?, ?, -1, ?);")
                                .bind(person_id).bind(person_name).bind(person_image).execute(&mut *begin).await.unwrap();
                            debug!(person_name);
                        }
                        _ => unreachable!()
                    }
                }
                page += 1;
                if card_array.len() != chunk_size { break; }
            }
        }
    }
    begin.commit().await.unwrap();
    info!("end updating cards...");
}


#[tracing::instrument(skip(semaphore))]
async fn update_card_belong(user_id: i64, semaphore: Arc<Semaphore>) {
    let _permit = semaphore.acquire().await.unwrap();
    info!("start updating card affiliation: {}...",user_id);
    let chunk_size = 25;
    let client = generate_client(false).await;

    let mut begin = DATABASE_POOL.get().unwrap().begin().await.unwrap();
    // sqlx::query("TRUNCATE TABLE cards;").execute(&mut *begin).await.unwrap();
    if sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM belong WHERE user_id = ?;").bind(user_id).fetch_one(DATABASE_POOL.get().unwrap()).await.unwrap().0 != 0 {
        sqlx::query("SELECT * FROM belong WHERE user_id = ? FOR UPDATE;").bind(user_id)
            .execute(&mut *begin).await.unwrap();
        sqlx::query("DELETE FROM belong WHERE user_id = ?;").bind(user_id)
            .execute(&mut *begin).await.unwrap();
    }

    for card_type in ["memorial", "non_memorial"] {
        for rarity in 1..=5 {
            let mut page = 1;
            loop {
                let query = format!("https://api-helloproject.orical.jp/cards?partner_id={PARTNER_ID}&user_id={user_id}&card_type={card_type}&ownership_type=owned&rarity={rarity}&page={page}&per={chunk_size}");
                let mut retry_val = 6;
                let resp = loop {
                    match client.get(&query).send().await.unwrap().json::<Value>().await {
                        Ok(x) => { break x; }
                        Err(_) => {
                            retry_val -= 1;
                            warn!("retrying...: {}",query);
                            sleep(Duration::new(30, 0)).await;
                        }
                    }
                    if retry_val == 0 { panic!() }
                };
                let card_array = resp.as_array().unwrap();
                debug!(query);
                for card in card_array {
                    let stat = card["card_users"].as_array().unwrap().iter().next().unwrap();
                    let card_id = stat["card_id"].as_i64().unwrap();
                    let is_protected = stat["is_protected"].as_bool().unwrap();
                    let unique_id = stat["id"].as_i64().unwrap();
                    let amount = stat["amount"].as_u64().unwrap();

                    debug!("card id: {}", card_id);
                    // debug!("\tstat: {}", stat);
                    debug!("\tis_protected: {}", is_protected);
                    debug!("\tunique_id: {}", unique_id);
                    debug!("\tamount: {}", amount);


                    sqlx::query("INSERT belong(user_id, card_id, unique_id, amount, protected) VALUES(?, ?, ?, ?, ?);")
                        .bind(user_id).bind(card_id).bind(unique_id).bind(amount).bind(is_protected)
                        .execute(&mut *begin).await.unwrap();
                }
                page += 1;
                if card_array.len() != chunk_size { break; }
            }
        }
    }
    begin.commit().await.unwrap();
    info!("end updating card affiliation: {}...",user_id);
}

async fn list_users() -> Vec<i64> {
    sqlx::query_as::<_, (i64,)>("SELECT user_id FROM orical_user;")
        .fetch_all(DATABASE_POOL.get().unwrap()).await.unwrap().into_iter().map(|v| { v.0 }).collect::<Vec<_>>()
}

async fn run_with_async_fn<F, Fut>(async_fn: F, semaphore: Arc<Semaphore>) -> Fut
where
    F: Fn() -> Fut,
    Fut: Future<Output=()>,
{
    let _ = semaphore.acquire().await.unwrap();
    async_fn()
}