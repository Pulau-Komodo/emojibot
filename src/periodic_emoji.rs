use serenity::{
	model::prelude::{Message, UserId},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{emoji::Emoji, queries::give_emoji, user_settings::private::is_private};

/// Period is currently one week.
async fn seen_this_period(database: &Pool<Sqlite>, user: UserId) -> bool {
	let user_id = user.get() as i64;
	// %G is ISO 8601 year corresponding to %V. %V is ISO 8601 week. It is basically a week that is not interrupted by year changes.
	let seen = query!(
		r#"
		SELECT CASE
			WHEN strftime('%G-%V', date) == strftime('%G-%V', date()) THEN true
			ELSE false
			END "seen_this_period!: bool"
		FROM last_seen
		WHERE user = ?
		"#,
		user_id
	)
	.fetch_optional(database)
	.await
	.unwrap()
	.map(|record| record.seen_this_period)
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
