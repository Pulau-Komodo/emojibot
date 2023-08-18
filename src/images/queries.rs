use serenity::model::prelude::UserId;
use sqlx::{query, Pool, Sqlite};

use crate::emojis_with_counts::EmojisWithCounts;

pub(super) async fn has_emojis(
	executor: &Pool<Sqlite>,
	user: UserId,
	emojis: EmojisWithCounts,
) -> bool {
	let user_id = user.0 as i64;

	let mut transaction = executor.begin().await.unwrap();

	for &(emoji, count) in &emojis {
		let emoji_str = emoji.as_str();
		let user_has_enough = query!(
			"
			SELECT COUNT(*) AS count
			FROM emoji_inventory
			WHERE user = ? AND emoji = ?
			",
			user_id,
			emoji_str
		)
		.fetch_one(&mut *transaction)
		.await
		.unwrap()
		.count as u32
			> count;

		if !user_has_enough {
			transaction.commit().await.unwrap();
			return false;
		}
	}

	transaction.commit().await.unwrap();
	true
}
