CREATE TABLE IF NOT EXISTS orical_user
(
    user_id          INT PRIMARY KEY,
    orical_id        INT  NOT NULL,
#     season_id        INT  NOT NULL,
    screen_name      TEXT NOT NULL,
    comment          TEXT,
    frontal_card_ids TEXT
);
CREATE INDEX IF NOT EXISTS idx_orical_user_user_id ON orical_user (user_id);

CREATE TABLE IF NOT EXISTS characters
(
    character_id   INT,
#     card_type      ENUM ('unit', 'person') NOT NULL,
    name           TEXT NULL,
    unit_member_id INT,
    image          TEXT NULL,
    PRIMARY KEY (character_id, unit_member_id)
);

CREATE TABLE IF NOT EXISTS cardpacks
(
    cardpack_id  INT PRIMARY KEY,
    name         TEXT,
    description  TEXT,
    available_at DATETIME,
    closes_at    DATETIME
);

CREATE TABLE IF NOT EXISTS cards
(
    card_id              INT PRIMARY KEY,
#     name                 TEXT                    NULL,
#     description          TEXT                    NULL,
#     cardpack_id          INT                     NOT NULL,
    memorial             INT                     NULL,
    rarity               INT                     NOT NULL,
    card_type            ENUM ('unit', 'person') NOT NULL,
    character_id         INT                     NOT NULL,
    season_id            INT                     NOT NULL,
    frontimage           TEXT,
    frontimage_thumbnail TEXT
#     ,FOREIGN KEY fk_character_id (character_id) REFERENCES characters (character_id) ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_card_id ON cards (card_id);
CREATE INDEX IF NOT EXISTS idx_character_id ON cards (character_id);
CREATE TABLE IF NOT EXISTS belong
(
    user_id   INT,
    card_id   INT          NOT NULL,
    unique_id INT          NOT NULL,
    amount    INT UNSIGNED NOT NULL,
    protected BOOL         NOT NULL,
    PRIMARY KEY (user_id, card_id)
#     ,FOREIGN KEY fk_user_id (user_id) REFERENCES orical_user (user_id) ON DELETE CASCADE ON UPDATE CASCADE
#     ,FOREIGN KEY fk_card_id (card_id) REFERENCES cards (card_id) ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_belong_user_id ON belong (user_id);


CREATE TABLE IF NOT EXISTS cardpack_belong
(
    cardpack_id INT NOT NULL,
    card_id     INT NOT NULL,
    PRIMARY KEY (cardpack_id, card_id)
);