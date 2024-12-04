use std::collections::HashMap;
use std::fs;
use reqwest;
use serde_json::Value;


#[derive(Debug)]
struct OricalContainer {
    custom_token: String,
    secure_token: String,
    user_id: i64,
    orical_id: i64,
    screen_name: String,
}
impl OricalContainer {
    async fn get_secure_token(custom_token: String) -> String {
        let client = reqwest::Client::new();
        client.post("https://identitytoolkit.googleapis.com/v1/accounts:signInWithCustomToken?key=AIzaSyABJQ_1lLpugYT2kuzdCsRmcx0P8QRG16s").
            json(&HashMap::from([("token", custom_token), ("returnSecureToken", "true".to_string())])).send().await.unwrap().json::<Value>().await.unwrap()["idToken"].as_str().unwrap().to_string()
    }
    async fn get_custom_token(login_id: HashMap<String, String>) -> String {
        let client = reqwest::Client::new();
        client.post("https://account-api.orical.jp/firebase_user/generate_custom_token?idprovider_key=helloproject_id").
            json(&login_id).send().await.unwrap().json::<Value>().await.unwrap()["custom_token"].as_str().unwrap().to_string()
    }
    async fn new(login_id: HashMap<String, String>) -> OricalContainer {
        let client = reqwest::Client::new();
        let custom_token = OricalContainer::get_custom_token(login_id).await;
        let secure_token = OricalContainer::get_secure_token(custom_token.clone()).await;
        let user_desc = client.get("https://api-helloproject.orical.jp/partner_users?partner_id=13")
            .header(reqwest::header::AUTHORIZATION.to_string(), secure_token.clone()).send().await.unwrap().json::<Value>().await.unwrap();
        let user_id = user_desc["user_id"].as_i64().unwrap();
        let orical_id = user_desc["orica"]["id"].as_i64().unwrap();
        let screen_name = user_desc["screen_name"].as_str().unwrap().to_string();

        OricalContainer { custom_token, secure_token, user_id, orical_id, screen_name }
    }

    // async fn has_bonus(&self) -> Value {
    //     let client = reqwest::Client::new();
    //     client.put("https://api-helloproject.orical.jp/loginbonuses/check?partner_id=13&user_id")
    //         .header(reqwest::header::AUTHORIZATION.to_string(), self.secure_token.clone(),
    //         ).send().await.unwrap().json::<Value>().await.unwrap()
    // }
    async fn card_counts(&self) -> HashMap<i64, (i64, i64)> {
        let client = reqwest::Client::new();
        client.get(format!("https://api-helloproject.orical.jp/card_users/count_by_stars?partner_id=13&screen_name={0}", self.screen_name)).send().await.unwrap().json::<Value>().await.unwrap()
            .as_array().unwrap().iter().enumerate().map(|(i, v)| (i as i64 + 1, (v["total"].as_i64().unwrap(), v["count"].as_i64().unwrap()))).collect()
    }
}
// https://api-helloproject.orical.jp/card_users/count_by_stars?partner_id=13&screen_name=yayoi_mizuha
#[tokio::main]
async fn main() {
    let login_id = serde_json::from_str::<Value>(fs::read_to_string("login_info.json").unwrap().as_str()).unwrap().as_object().unwrap().iter().map(|(k, v)| {
        (k.clone(), v.clone().as_str().unwrap().to_string())
    }).collect::<HashMap<String, String>>();

    let orical_container = OricalContainer::new(login_id).await;

    // println!("has bonus?: {}", orical_container.has_bonus().await);
    println!("{:?}", orical_container.card_counts().await);
}
