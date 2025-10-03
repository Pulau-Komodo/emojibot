use std::sync::Arc;

use serenity::{
	all::{CacheHttp, RoleId},
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
	pub trading_roles: &'l Vec<RoleId>,
	pub http: &'l Arc<Http>,
	pub cache: &'l Arc<Cache>,
}

impl<'l> Context<'l> {
	pub fn new(
		database: &'l Pool<Sqlite>,
		emoji_map: &'l EmojiMap,
		trading_roles: &'l Vec<RoleId>,
		http: &'l Arc<Http>,
		cache: &'l Arc<Cache>,
	) -> Self {
		Self {
			database,
			emoji_map,
			trading_roles,
			http,
			cache,
		}
	}

	/// Gives nickname if possible, otherwise display name, otherwise ID as a string.
	pub async fn get_user_name(&self, guild: GuildId, user: UserId) -> String {
		let member = if let Some(member) = self
			.cache
			.guild(guild)
			.and_then(|guild| guild.members.get(&user).cloned())
		{
			member
		} else if let Ok(member) = self.http.get_member(guild, user).await {
			member
		} else {
			return format!("{}", user);
		};
		if let Some(nick) = member.nick {
			nick
		} else {
			member.display_name().to_owned()
		}
	}
}

impl CacheHttp for Context<'_> {
	fn http(&self) -> &Http {
		self.http
	}
	fn cache(&self) -> Option<&Arc<Cache>> {
		Some(self.cache)
	}
}
