use serenity::{
	model::prelude::{Message, UserId},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{emoji::Emoji, queries::give_emoji, user_settings::private::is_private};

/// Period is currently one week.
/// To do: replace date comparison with less janky %G-%V implementation when SQLx supports SQLite 3.46.
async fn seen_this_period(database: &Pool<Sqlite>, user: UserId) -> bool {
	let user_id = user.get() as i64;
	let seen = query!(
		r#"
		SELECT CASE
			WHEN cast(strftime('%j', date) / 7 as int) == cast(strftime('%j', date()) / 7 as int) THEN true
			ELSE false
			END "seen_today!: bool"
		FROM last_seen
		WHERE user = ?
		"#,
		user_id
	)
	.fetch_optional(database)
	.await
	.unwrap()
	.map(|record| record.seen_today)
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

pub async fn maybe_give_periodic_emoji(
	database: &Pool<Sqlite>,
	context: Context,
	message: Message,
) {
	if !seen_this_period(database, message.author.id).await {
		let emoji = Emoji::random();
		give_emoji(database, message.author.id, emoji).await;
		if !is_private(database, message.author.id).await {
			let _ = message.react(context, emoji).await;
		}
	}
}
