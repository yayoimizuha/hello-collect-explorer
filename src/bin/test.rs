use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use chrono::{DateTime, FixedOffset};
use reqwest;
use reqwest::Client;
use serde_json::Value;
use serde::{Deserialize, Serialize};

static PARTNER_ID: i32 = 13;
#[derive(Debug)]
struct OricalUserContainer {
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

impl OricalUserContainer {
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
    async fn from_credential(login_id: HashMap<String, String>) -> OricalUserContainer {
        eprintln!("login processing...");
        let client = reqwest::Client::new();
        let custom_token = OricalUserContainer::get_custom_token(login_id).await;
        let secure_token = OricalUserContainer::get_secure_token(custom_token.clone()).await;
        let user_desc = client.get(format!("https://api-helloproject.orical.jp/partner_users?partner_id={PARTNER_ID}"))
            .header(reqwest::header::AUTHORIZATION.to_string(), secure_token.clone()).send().await.unwrap().json::<Value>().await.unwrap();
        let user_id = user_desc["user_id"].as_i64().unwrap();
        let orical_id = user_desc["orica"]["id"].as_i64().unwrap();
        let season_id = user_desc["partner"]["current_season_id"].as_i64().unwrap();
        let screen_name = user_desc["screen_name"].as_str().unwrap().to_string();

        OricalUserContainer { custom_token, secure_token, user_id, orical_id, season_id, screen_name }
    }
    async fn new(custom_token: String, secure_token: String, user_id: Option<i64>, screen_name: Option<String>) -> OricalUserContainer {
        let user_info = Client::new().get(format!("https://api-helloproject.orical.jp/partner_users?partner_id={PARTNER_ID}&{0}", match (user_id, screen_name) {
            (Some(x), _) => { format!("user_id={x}") }
            (_, Some(x)) => { format!("screen_name={x}") }
            (None, None) => { panic!() }
        })).header(reqwest::header::AUTHORIZATION.to_string(), secure_token.clone()).send().await.unwrap().json::<Value>().await.unwrap();
        OricalUserContainer {
            custom_token,
            secure_token,
            user_id: user_info["user_id"].as_i64().unwrap(),
            orical_id: user_info["orica"]["id"].as_i64().unwrap(),
            season_id: user_info["partner"]["current_season_id"].as_i64().unwrap(),
            screen_name: user_info["screen_name"].as_str().unwrap().to_string(),
        }
    }

    async fn card_counts(&self) -> HashMap<i64, (i64, i64)> {
        eprintln!("getting card counts...");
        let client = reqwest::Client::new();
        client.get(format!("https://api-helloproject.orical.jp/card_users/count_by_stars?partner_id={PARTNER_ID}&screen_name={0}", self.screen_name)).send().await.unwrap().json::<Value>().await.unwrap()
            .as_array().unwrap().iter().enumerate().map(|(i, v)| (i as i64 + 1, (v["total"].as_i64().unwrap(), v["count"].as_i64().unwrap()))).collect()
    }
    async fn last_update_time(&self) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(Client::new().get(format!(
            "https://api-helloproject.orical.jp/card_users?page=1&per=1&partner_id={PARTNER_ID}&season_id={0}&screen_name={1}&card_type=non_memorial&order=created_at",
            self.season_id, self.screen_name)).send().await.unwrap().json::<Value>().await.unwrap()[0]["updated_at"].as_str().unwrap()).unwrap()
    }

    async fn card_listing(&self) -> HashMap<i64, HashSet<CardContainer>> {
        eprintln!("getting cards...");
        let client = reqwest::Client::new();
        let mut card_list = HashMap::<i64, HashSet<CardContainer>>::new();
        for (rarity, (_, cards_count)) in self.card_counts().await {
            for i in 0..(cards_count as f32 / 100.0).ceil() as i64 {
                for card in client.get(format!("https://api-helloproject.orical.jp/cards?partner_id={PARTNER_ID}&season_id={0}&user_id={1}&card_type=non_memorial&ownership_type=owned&rarity={2}&page={3}&per=100",
                                               self.season_id, self.user_id, rarity, i)).send().await.unwrap().json::<Value>().await.unwrap().as_array().unwrap() {
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

    let orical_container = OricalUserContainer::from_credential(login_id).await;

    println!("{:?}", orical_container.card_counts().await);
    // println!("{}", serde_json::to_string_pretty(&orical_container.card_listing(orical_container.user_id).await).unwrap());

    // println!("{}", orical_container.secure_token);


    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, reqwest::header::HeaderValue::from_str(&orical_container.secure_token).unwrap());
    headers.get_mut(reqwest::header::AUTHORIZATION).unwrap().set_sensitive(true);
    let client = reqwest::Client::builder().default_headers(headers).build().unwrap();
    let unauthorized_client = reqwest::Client::new();
    println!("presentboxes/check_if_unreceived_exists?context=actionbonus : \n{}",
             client.get(format!("https://api-helloproject.orical.jp/presentboxes/check_if_unreceived_exists?context=actionbonus&partner_id={PARTNER_ID}")).send().await.unwrap().text().await.unwrap());

    if false {
        let mut login_bonus_check = client.put(format!("https://api-helloproject.orical.jp/loginbonuses/check?partner_id={PARTNER_ID}")).send().await.unwrap().json::<Value>().await.unwrap();
        let _ = login_bonus_check.as_array_mut().unwrap().into_iter().map(|v| { v.as_object_mut().unwrap().remove("loginrewards") }).count();
        println!("loginbonuses/check : \n{}", serde_json::to_string_pretty(&login_bonus_check).unwrap());
    }
    if false {
        let mut partner_users = client.get(format!("https://api-helloproject.orical.jp/partner_users?partner_id={PARTNER_ID}")).send().await.unwrap().json::<Value>().await.unwrap();
        let _ = partner_users.as_object_mut().unwrap().remove("orica").unwrap();
        println!("partner_users : \n{}", serde_json::to_string_pretty(&partner_users).unwrap());
    }

    let all_users_count = client.get(format!("https://api-helloproject.orical.jp/partners/{PARTNER_ID}/ranking/top100?page=1&per=1")).send().await.unwrap().json::<Value>().await.unwrap()["my_rank"]["num_rivals"].as_i64().unwrap();
    println!("{}", all_users_count);
    let all_users_count = 200_i64;
    for i in 0..=(all_users_count as f64 / 100.0).ceil() as i32 {
        for rank in client.get(format!("https://api-helloproject.orical.jp/partners/{PARTNER_ID}/ranking/top100?page={i}&per=100")).send().await.unwrap().json::<Value>().await.unwrap()["rankings"].as_array().unwrap() {
            let screen_name = rank["partner_user"]["screen_name"].as_str().unwrap();
            let user_id = rank["partner_user"]["user_id"].as_i64().unwrap();
            println!("{}位\tuser_id:{user_id}\tscreen_name:{screen_name:<15}\tscore:{:<7}\tcollect:{}\ttotal:{}",
                     rank["rank"].as_i64().unwrap(),
                     rank["score"].as_i64().unwrap(),
                     rank["partner_user"]["collect"].as_i64().unwrap(),
                     rank["partner_user"]["total_cards_amount"].as_i64().unwrap());
            // let user = OricalUserContainer::new(orical_container.custom_token.clone(), orical_container.secure_token.clone(), Some(user_id), None).await;
            // for (rarity, cards) in user.card_listing().await {
            //     println!("\t星{rarity}");
            //     for card in cards {
            //         println!("\t\t{:?}",card.card_type);
            //     }
            // }
        }
    }


    //https://api-helloproject.orical.jp/partner_users?partner_id=13
}
