#![allow(clippy::get_first)]

use std::fs;

use discord_events::DiscordEventHandler;
use emoji::EmojiMap;
use serenity::prelude::GatewayIntents;
use sqlx::sqlite::SqlitePoolOptions;

mod context;
mod discord_events;
mod emoji;
mod emoji_list;
mod emojis_with_counts;
mod find_emoji;
mod images;
mod inventory;
mod periodic_emoji;
mod queries;
mod special_characters;
mod trading;
mod user_settings;
mod util;

#[tokio::main]
async fn main() {
	let discord_token =
		fs::read_to_string("./discord_token.txt").expect("Could not read Discord token file");

	let db_pool = SqlitePoolOptions::new()
		.max_connections(4)
		.connect("./data/db.db")
		.await
		.unwrap();

	let version = sqlx::query!("SELECT sqlite_version() as version;")
		.fetch_one(&db_pool)
		.await
		.unwrap();
	println!("SQLite version {}", version.version.unwrap());

	let emoji_map = EmojiMap::load();

	let handler = DiscordEventHandler::new(db_pool, emoji_map);
	let mut client = serenity::Client::builder(
		&discord_token,
		GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES,
	)
	.event_handler(handler)
	.await
	.expect("Error creating Discord client");

	if let Err(why) = client.start().await {
		eprintln!("Error starting client: {:?}", why);
	}
}
