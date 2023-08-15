use serenity::model::prelude::UserId;
use sqlx::{query, Pool, Sqlite};

use crate::emoji::{Emoji, EmojiMap};

/// Check a user's emoji inventory to see if it has the emojis.
pub(crate) async fn does_user_have_emojis(
	executor: &Pool<Sqlite>,
	user: UserId,
	emojis: &[(Emoji, i64)],
) -> bool {
	let user_id = user.0 as i64;
	let mut transaction = executor.begin().await.unwrap();
	for (emoji, target_count) in emojis {
		let emoji = emoji.as_str();
		let count = query!(
			"
				SELECT
					count
				FROM
					emoji_inventory
				WHERE
					user = ? AND emoji = ?
				",
			user_id,
			emoji
		)
		.fetch_optional(&mut *transaction)
		.await
		.unwrap()
		.map(|record| record.count)
		.unwrap_or(0);
		if count < *target_count {
			transaction.commit().await.unwrap();
			return false;
		}
	}
	transaction.commit().await.unwrap();
	true
}

pub async fn get_user_emojis(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	user: UserId,
) -> Vec<(Emoji, i64)> {
	let user_id = *user.as_u64() as i64;
	let mut emojis = query!(
		"
		SELECT
			emoji, count
		FROM
			emoji_inventory
		WHERE
			user = ?
		",
		user_id
	)
	.fetch_all(database)
	.await
	.unwrap()
	.into_iter()
	.filter_map(|record| {
		(record.count > 0).then_some((
			*emoji_map
				.get(record.emoji.as_str())
				.expect("Emoji from database was somehow not in map."),
			record.count,
		))
	})
	.collect::<Vec<_>>();
	emojis.sort_unstable();
	emojis
}
