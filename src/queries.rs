use std::collections::HashMap;

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
			SELECT COUNT(*) AS count
			FROM emoji_inventory
			WHERE user = ? AND emoji = ?
			",
			user_id,
			emoji
		)
		.fetch_optional(&mut *transaction)
		.await
		.unwrap()
		.map(|record| record.count)
		.unwrap_or(0);
		if (count as i64) < *target_count {
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
		SELECT emoji, COUNT(*) AS count
		FROM emoji_inventory
		WHERE user = ?
		GROUP BY emoji
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

pub async fn get_user_emojis_grouped(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	user: UserId,
) -> Vec<Vec<(Emoji, i64)>> {
	let user_id = user.0 as i64;
	let records = query!(
		"
		SELECT emoji, COUNT(*) AS count, sort_order
		FROM emoji_inventory
		LEFT JOIN emoji_inventory_groups
		ON emoji_inventory.group_id = emoji_inventory_groups.id
		WHERE emoji_inventory.user = ?
		GROUP BY emoji_inventory.user, emoji, group_id
		",
		user_id
	)
	.fetch_all(database)
	.await
	.unwrap();

	let mut emoji_groups = HashMap::<i64, Vec<(Emoji, i64)>>::new();
	for record in records {
		let sort_order = record.sort_order.unwrap_or(i64::MAX);
		if record.count > 0 {
			let emoji = *emoji_map
				.get(record.emoji.as_str())
				.expect("Emoji from database was somehow not in map.");
			emoji_groups
				.entry(sort_order)
				.or_default()
				.push((emoji, record.count));
		}
	}
	for group in &mut emoji_groups {
		group.1.sort_unstable();
	}
	let mut emoji_groups = emoji_groups.into_iter().collect::<Vec<_>>();
	emoji_groups.sort_unstable_by_key(|(sort, _)| *sort);
	emoji_groups.into_iter().map(|(_, emojis)| emojis).collect()
}
