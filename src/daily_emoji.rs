use serenity::{
	model::prelude::{Message, UserId},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{emoji::Emoji, queries::give_emoji, user_settings::private::is_private};

async fn seen_today(database: &Pool<Sqlite>, user: UserId) -> bool {
	let user_id = *user.as_u64() as i64;
	let seen = query!(
		"
		SELECT CASE
			WHEN date >= date() THEN true
			ELSE false
			END seen_today
		FROM last_seen
		WHERE user = ?
		",
		user_id
	)
	.fetch_optional(database)
	.await
	.unwrap()
	.map(|record| record.seen_today != 0)
	.unwrap_or(false);
	if !seen {
		query!(
			"
			INSERT INTO last_seen (user)
			VALUES (?)
			",
			user_id
		)
		.execute(database)
		.await
		.unwrap();
	}
	seen
}

pub async fn maybe_give_daily_emoji(database: &Pool<Sqlite>, context: Context, message: Message) {
	if !seen_today(database, message.author.id).await {
		let emoji = Emoji::random();
		give_emoji(database, message.author.id, emoji).await;
		if !is_private(database, message.author.id).await {
			let _ = message.react(context, emoji).await;
		}
	}
}
