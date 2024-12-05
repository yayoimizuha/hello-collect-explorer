use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use reqwest;
use serde_json::Value;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
struct OricalContainer {
    custom_token: String,
    secure_token: String,
    user_id: i64,
    orical_id: i64,
    season_id: i64,
    screen_name: String,
}
#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
enum CardType {
    Person((String, i64)),
    Unit((String, i64, HashSet<(String, i64)>)),
}
impl Hash for CardType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            CardType::Person(x) => { x.clone().hash(state); }
            CardType::Unit(v) => {
                let v2 = v.2.clone();
                let mut sorted_people = v2.iter().collect::<Vec<_>>();
                sorted_people.sort();
                (v.0.clone(), v.1.clone(), sorted_people).hash(state);
            }
        }
    }
}
#[derive(Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
struct CardContainer {
    card_type: CardType,
    card_id: i64,
    unit_id: Option<i64>,
    person_id: Option<i64>,
    name: String,
    description: String,
    amount: i64,
    rarity: i64,
    protected: bool,
    frontimage: String,
    frontimage_thumbnail: String,
}

impl OricalContainer {
    async fn get_secure_token(custom_token: String) -> String {
        eprintln!("getting secure token...");
        let client = reqwest::Client::new();
        client.post("https://identitytoolkit.googleapis.com/v1/accounts:signInWithCustomToken?key=AIzaSyABJQ_1lLpugYT2kuzdCsRmcx0P8QRG16s").
            json(&HashMap::from([("token", custom_token), ("returnSecureToken", "true".to_string())])).send().await.unwrap().json::<Value>().await.unwrap()["idToken"].as_str().unwrap().to_string()
    }
    async fn get_custom_token(login_id: HashMap<String, String>) -> String {
        eprintln!("getting custom token...");
        let client = reqwest::Client::new();
        client.post("https://account-api.orical.jp/firebase_user/generate_custom_token?idprovider_key=helloproject_id").
            json(&login_id).send().await.unwrap().json::<Value>().await.unwrap()["custom_token"].as_str().unwrap().to_string()
    }
    //noinspection SpellCheckingInspection
    async fn new(login_id: HashMap<String, String>) -> OricalContainer {
        eprintln!("login processing...");
        let client = reqwest::Client::new();
        let custom_token = OricalContainer::get_custom_token(login_id).await;
        let secure_token = OricalContainer::get_secure_token(custom_token.clone()).await;
        let user_desc = client.get("https://api-helloproject.orical.jp/partner_users?partner_id=13")
            .header(reqwest::header::AUTHORIZATION.to_string(), secure_token.clone()).send().await.unwrap().json::<Value>().await.unwrap();
        let user_id = user_desc["user_id"].as_i64().unwrap();
        let orical_id = user_desc["orica"]["id"].as_i64().unwrap();
        let season_id = user_desc["partner"]["current_season_id"].as_i64().unwrap();
        let screen_name = user_desc["screen_name"].as_str().unwrap().to_string();

        OricalContainer { custom_token, secure_token, user_id, orical_id, season_id, screen_name }
    }

    async fn card_counts(&self) -> HashMap<i64, (i64, i64)> {
        eprintln!("getting card counts...");
        let client = reqwest::Client::new();
        client.get(format!("https://api-helloproject.orical.jp/card_users/count_by_stars?partner_id=13&screen_name={0}", self.screen_name)).send().await.unwrap().json::<Value>().await.unwrap()
            .as_array().unwrap().iter().enumerate().map(|(i, v)| (i as i64 + 1, (v["total"].as_i64().unwrap(), v["count"].as_i64().unwrap()))).collect()
    }

    async fn card_listing(&self, user_id: i64) -> HashMap<i64, HashSet<CardContainer>> {
        eprintln!("getting cards...");
        let client = reqwest::Client::new();
        let mut card_list = HashMap::<i64, HashSet<CardContainer>>::new();
        for (rarity, (_, cards_count)) in self.card_counts().await {
            for i in 0..(cards_count as f32 / 100.0).ceil() as i64 {
                for card in client.get(format!("https://api-helloproject.orical.jp/cards?partner_id=13&season_id={0}&user_id={1}&card_type=non_memorial&ownership_type=owned&rarity={2}&page={3}&per=100",
                                               self.season_id, user_id, rarity, i)).send().await.unwrap().json::<Value>().await.unwrap().as_array().unwrap() {
                    // println!("{}", serde_json::to_string(card).unwrap());
                    match card_list.get(&rarity) {
                        None => { card_list.insert(rarity, HashSet::new()); }
                        Some(_) => {}
                    }
                    card_list.get_mut(&rarity).unwrap().insert(CardContainer {
                        card_type: if card.clone().as_object().unwrap().contains_key("unit") {
                            CardType::Unit((card["unit"]["name"].as_str().unwrap().to_string(), card["unit"]["id"].as_i64().unwrap(),
                                            card["unit"]["people"].as_array().unwrap().iter().map(|p| {
                                                (p["name"].as_str().unwrap().to_string(), p["id"].as_i64().unwrap())
                                            }).collect()))
                        } else {
                            CardType::Person((card["person"]["name"].as_str().unwrap().to_string(),
                                              card["person"]["id"].as_i64().unwrap()))
                        },
                        card_id: card["id"].as_i64().unwrap(),
                        unit_id: if card.clone().as_object().unwrap().contains_key("unit") {
                            Some(card["unit_id"].as_i64().unwrap())
                        } else { None },
                        person_id: if !card.clone().as_object().unwrap().contains_key("unit") {
                            Some(card["person_id"].as_i64().unwrap())
                        } else { None },
                        name: card["name"].as_str().unwrap().to_string(),
                        description: card["description"].as_str().unwrap().to_string(),
                        amount: card["card_users"][0]["amount"].as_i64().unwrap(),
                        rarity: card["rarity"].as_i64().unwrap(),
                        protected: card["card_users"][0]["is_protected"].as_bool().unwrap(),
                        frontimage: card["frontimage"].as_str().unwrap().to_string(),
                        frontimage_thumbnail: card["frontimage_thumbnail"].as_str().unwrap().to_string(),
                    });
                }
            }
        }
        card_list
    }
}

#[tokio::main]
async fn main() {
    let login_id = serde_json::from_str::<Value>(fs::read_to_string("login_info.json").unwrap().as_str()).unwrap().as_object().unwrap().iter().map(|(k, v)| {
        (k.clone(), v.clone().as_str().unwrap().to_string())
    }).collect::<HashMap<String, String>>();

    let orical_container = OricalContainer::new(login_id).await;

    println!("{:?}", orical_container.card_counts().await);
    println!("{}", serde_json::to_string_pretty(&orical_container.card_listing(orical_container.user_id).await).unwrap());
}
