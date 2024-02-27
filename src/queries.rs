use std::collections::HashMap;

use serenity::model::prelude::UserId;
use sqlx::{query, Pool, Sqlite, SqliteExecutor};

use crate::{
	emoji::{Emoji, EmojiMap},
	emojis_with_counts::EmojisWithCounts,
};

pub async fn get_user_emojis_grouped(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	user: UserId,
) -> (Vec<EmojisWithCounts>, Option<EmojisWithCounts>) {
	let user_id = user.get() as i64;
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

	let mut emoji_groups = HashMap::<Option<u32>, Vec<(Emoji, u32)>>::new();
	for record in records {
		let sort_order = record.sort_order.map(|n| n as u32);
		if record.count > 0 {
			let emoji = emoji_map
				.get(record.emoji.as_str())
				.expect("Emoji from database was somehow not in map.");
			emoji_groups
				.entry(sort_order)
				.or_default()
				.push((emoji.emoji(), record.count as u32));
		}
	}

	let ungrouped = emoji_groups.remove(&None).map(EmojisWithCounts::new);

	let mut emoji_groups = emoji_groups.into_iter().collect::<Vec<_>>();
	emoji_groups.sort_unstable_by_key(|(sort, _)| *sort);
	let emoji_groups = emoji_groups
		.into_iter()
		.map(|(_ready, group)| EmojisWithCounts::new(group))
		.collect();

	(emoji_groups, ungrouped)
}

pub async fn give_emoji<'c, E: SqliteExecutor<'c>>(database: E, user: UserId, emoji: Emoji) {
	let user_id = user.get() as i64;
	let emoji = emoji.as_str();
	query!(
		"
		INSERT INTO emoji_inventory (user, emoji)
		VALUES (?, ?)
		",
		user_id,
		emoji
	)
	.execute(database)
	.await
	.unwrap();
}
