--
-- File generated with SQLiteStudio v3.4.4 on Thu Aug 3 02:03:38 2023
--
-- Text encoding used: System
--
PRAGMA foreign_keys = off;
BEGIN TRANSACTION;

-- Table: emoji_inventory
CREATE TABLE IF NOT EXISTS emoji_inventory (user INTEGER NOT NULL, emoji TEXT NOT NULL, count INTEGER NOT NULL DEFAULT (1), PRIMARY KEY (user, emoji) ON CONFLICT ROLLBACK) WITHOUT ROWID;

-- Table: last_seen
CREATE TABLE IF NOT EXISTS last_seen (user NUMERIC PRIMARY KEY UNIQUE ON CONFLICT REPLACE NOT NULL, date DATE NOT NULL DEFAULT (date()));

-- Table: trade_log
CREATE TABLE IF NOT EXISTS trade_log (id INTEGER PRIMARY KEY, initiating_user INTEGER NOT NULL, recipient_user INTEGER NOT NULL, time DATETIME DEFAULT (datetime()) NOT NULL);

-- Table: trade_log_contents
CREATE TABLE IF NOT EXISTS trade_log_contents (trade NOT NULL REFERENCES trade_log (id) ON DELETE CASCADE ON UPDATE CASCADE, emoji TEXT NOT NULL, count INTEGER NOT NULL CHECK (count != 0));

-- Table: trade_offer_contents
CREATE TABLE IF NOT EXISTS trade_offer_contents (trade INTEGER REFERENCES trade_offers (id) ON DELETE CASCADE ON UPDATE CASCADE NOT NULL, emoji TEXT NOT NULL, count INTEGER NOT NULL CHECK (count != 0));

-- Table: trade_offers
CREATE TABLE IF NOT EXISTS trade_offers (id INTEGER PRIMARY KEY NOT NULL, user INTEGER NOT NULL, target_user INTEGER NOT NULL, time DATETIME DEFAULT (datetime()) NOT NULL);

-- Index: 
CREATE UNIQUE INDEX IF NOT EXISTS "" ON trade_offers (user, target_user);

COMMIT TRANSACTION;
PRAGMA foreign_keys = on;
