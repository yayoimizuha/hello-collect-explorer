CREATE TABLE IF NOT EXISTS orical_user
(
    user_id          INT PRIMARY KEY,
    orical_id        INT  NOT NULL,
    season_id        INT  NOT NULL,
    screen_name      TEXT NOT NULL,
    comment          TEXT,
    frontal_card_ids TEXT
);
CREATE INDEX IF NOT EXISTS idx_user_id ON orical_user (user_id);
CREATE TABLE IF NOT EXISTS cards
(
    card_id              INT PRIMARY KEY,
    name                 TEXT                    NULL,
    description          TEXT                    NULL,
    rarity               INT                     NOT NULL,
    card_type            ENUM ('unit', 'person') NOT NULL,
    character_id         INT                     NOT NULL,
    season_id            INT                     NOT NULL,
    frontimage           TEXT,
    frontimage_thumbnail TEXT
);
CREATE INDEX IF NOT EXISTS idx_card_id ON cards (card_id);
CREATE INDEX IF NOT EXISTS idx_character_id ON cards (character_id);
CREATE TABLE IF NOT EXISTS belong
(
    user_id   INT PRIMARY KEY,
    amount    INT UNSIGNED NOT NULL,
    protected BOOL         NOT NULL
);
CREATE TABLE IF NOT EXISTS characters
(
    character_id   INT PRIMARY KEY,
    unit_name      TEXT NULL,
    unit_member_id INT  NULL,
    person_name    TEXT NULL,
    person_image   TEXT NULL
);