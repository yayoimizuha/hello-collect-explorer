use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::fs;
use reqwest;
use serde_json::Value;

static ACCESS_TOKEN: OnceCell<String> = OnceCell::new();

async fn generate_secure_token(login_id: HashMap<String, String>) -> Value {
    let client = reqwest::Client::new();
    let custom_token = client.post("https://account-api.orical.jp/firebase_user/generate_custom_token?idprovider_key=helloproject_id").
        json(&login_id).send().await.unwrap().json::<serde_json::Value>().await.unwrap()
        ["custom_token"].as_str().unwrap().to_string();
    client.post("https://identitytoolkit.googleapis.com/v1/accounts:signInWithCustomToken?key=AIzaSyABJQ_1lLpugYT2kuzdCsRmcx0P8QRG16s").
        json(&HashMap::from([("token", custom_token), ("returnSecureToken", "true".to_string())])).send().await.unwrap().json::<serde_json::Value>().await.unwrap()
}
#[tokio::main]
async fn main() {
    let login_id = serde_json::from_str::<Value>(fs::read_to_string("login_info.json").unwrap().as_str()).unwrap().as_object().unwrap().iter().map(|(k, v)| {
        (k.clone(), v.clone().as_str().unwrap().to_string())
    }).collect::<HashMap<String, String>>();

    let client = reqwest::Client::new();
    ACCESS_TOKEN.set(generate_secure_token(login_id).await["idToken"].as_str().unwrap().to_string()).unwrap();
    println!("{}", ACCESS_TOKEN.get().unwrap());

    println!("{}", client.get("https://api-helloproject.orical.jp/presentboxes/check_if_unreceived_exists?context=actionbonus&partner_id=13")
        .header(reqwest::header::AUTHORIZATION.to_string(), ACCESS_TOKEN.get().unwrap()
        ).send().await.unwrap().text().await.unwrap());
}
