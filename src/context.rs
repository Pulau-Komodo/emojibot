use std::sync::Arc;

use serenity::{
	client::Cache,
	http::Http,
	model::prelude::{GuildId, UserId},
};
use sqlx::{Pool, Sqlite};

use crate::emoji::EmojiMap;

#[derive(Copy, Clone)]
pub struct Context<'l> {
	pub database: &'l Pool<Sqlite>,
	pub emoji_map: &'l EmojiMap,
	pub http: &'l Arc<Http>,
	pub cache: &'l Arc<Cache>,
}

impl<'l> Context<'l> {
	pub fn new(
		database: &'l Pool<Sqlite>,
		emoji_map: &'l EmojiMap,
		http: &'l Arc<Http>,
		cache: &'l Arc<Cache>,
	) -> Self {
		Self {
			database,
			emoji_map,
			http,
			cache,
		}
	}

	/// Gives nickname if possible, otherwise display name, otherwise ID as a string.
	pub async fn get_user_name(&self, guild: GuildId, user: UserId) -> String {
		let member = if let Some(member) = self.cache.member(guild, user) {
			member
		} else if let Ok(member) = self.http.get_member(guild.0, user.0).await {
			member
		} else {
			return format!("{}", user.0);
		};
		if let Some(nick) = member.nick {
			nick
		} else {
			member.display_name().to_string()
		}
	}
}
