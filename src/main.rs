use sqlx::{mysql::MySqlPoolOptions, MySql, Pool};

#[tokio::main]
async fn main() {
    println!(env!("DATABASE_URL"));
    let database_pool = MySqlPoolOptions::new().connect(env!("DATABASE_URL")).await.unwrap();
    sqlx::query("CREATE TABLE IF NOT EXISTS orical_user(user_id INT PRIMARY KEY, orical_id INT NOT NULL, season_id INT NOT NULL, screen_name TEXT NOT NULL);").execute(&database_pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_user_id ON orical_user(user_id);").execute(&database_pool).await.unwrap();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS cards(card_id INT PRIMARY KEY, name TEXT NULL, description TEXT NULL, rarity INT NOT NULL,\
                                             card_type ENUM('unit', 'person') NOT NULL,character_id INT NOT NULL, season_id INT NOT NULL,\
                                             frontimage TEXT,frontimage_thumbnail TEXT);"
    ).execute(&database_pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_card_id ON cards(card_id);").execute(&database_pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_character_id ON cards(character_id);").execute(&database_pool).await.unwrap();
    sqlx::query("CREATE TABLE IF NOT EXISTS belong(user_id INT PRIMARY KEY, amount INT UNSIGNED NOT NULL, protected BOOL NOT NULL);").execute(&database_pool).await.unwrap();
    sqlx::query("CREATE TABLE IF NOT EXISTS character(character_id INT PRIMARY KEY,amount INT UNSIGNED NOT NULL);").execute(&database_pool).await.unwrap();

}
