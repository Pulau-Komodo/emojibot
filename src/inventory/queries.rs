use std::cmp::Ordering;

use serenity::model::prelude::UserId;
use sqlx::{query, Pool, Sqlite, SqliteExecutor, Transaction};

use crate::{emoji::EmojiMap, emojis_with_counts::EmojisWithCounts};

pub async fn remove_empty_groups(executor: &mut Transaction<'_, Sqlite>, user: UserId) {
	let user_id = user.get() as i64;
	let deleted_any = query!(
		"
		DELETE FROM emoji_inventory_groups
		WHERE user = ? AND (
			SELECT COUNT(*)
			FROM emoji_inventory
			WHERE emoji_inventory.user = emoji_inventory_groups.user AND emoji_inventory.group_id = emoji_inventory_groups.id
		) = 0
		", user_id
	).execute(&mut **executor).await.unwrap().rows_affected() > 0;
	if deleted_any {
		close_ordering_gaps(executor, user).await;
	}
}

async fn close_ordering_gaps(executor: &mut Transaction<'_, Sqlite>, user: UserId) {
	let user_id = user.get() as i64;
	let groups = query!(
		"
		SELECT name
		FROM emoji_inventory_groups
		WHERE user = ?
		ORDER BY sort_order ASC
		",
		user_id
	)
	.fetch_all(&mut **executor)
	.await
	.unwrap();

	for (index, group) in groups.into_iter().enumerate() {
		let group_name = group.name;
		let sort_order = index as i64;
		query!(
			"
			UPDATE emoji_inventory_groups
			SET sort_order = ? + 1
			WHERE user = ? AND name = ?
			",
			sort_order,
			user_id,
			group_name
		)
		.execute(&mut **executor)
		.await
		.unwrap();
	}
}

/// Returns the group name, the emojis successfully added to that group, and whether the group was newly made.
pub(super) async fn add_to_group(
	executor: &Pool<Sqlite>,
	user: UserId,
	group_name: &str,
	emojis: &EmojisWithCounts,
) -> (String, EmojisWithCounts, bool) {
	let user_id = user.get() as i64;
	let mut transaction = executor.begin().await.unwrap();
	let group_count = query!(
		"
		SELECT COUNT(*) AS group_count
		FROM emoji_inventory_groups
		WHERE user = ?
		",
		user_id
	)
	.fetch_one(&mut *transaction)
	.await
	.unwrap()
	.group_count;

	let group_is_new = query!(
		"
		INSERT INTO emoji_inventory_groups (user, name, sort_order)
		VALUES (?, ?, ? + 1)
		ON CONFLICT (user, name COLLATE NOCASE) DO NOTHING;
		",
		user_id,
		group_name,
		group_count
	)
	.execute(&mut *transaction)
	.await
	.unwrap()
	.rows_affected()
		> 0;

	let group = query!(
		"
		SELECT id, name
		FROM emoji_inventory_groups
		WHERE user = ? AND name = ?;
		",
		user_id,
		group_name,
	)
	.fetch_one(&mut *transaction)
	.await
	.unwrap();

	let group_id = group.id;
	let mut added_emojis = Vec::with_capacity(emojis.unique_emoji_count());
	for (emoji, count) in emojis {
		let emoji_str = emoji.as_str();
		let rows_affected = query!(
			"
			UPDATE emoji_inventory
			SET group_id = ?
			WHERE user = ? AND emoji = ? AND rowid IN (
				SELECT emoji_inventory.rowid
				FROM emoji_inventory
				LEFT JOIN emoji_inventory_groups
				ON emoji_inventory.group_id = emoji_inventory_groups.id
				WHERE emoji_inventory.user = ? AND emoji_inventory.emoji = ?
				ORDER BY IFNULL(sort_order, 9223372036854775807) DESC
				LIMIT ?
			)
			",
			group_id,
			user_id,
			emoji_str,
			user_id,
			emoji_str,
			*count
		)
		.execute(&mut *transaction)
		.await
		.unwrap()
		.rows_affected();
		if rows_affected > 0 {
			added_emojis.push((*emoji, rows_affected as u32));
		}
	}

	remove_empty_groups(&mut transaction, user).await;

	transaction.commit().await.unwrap();

	(
		group.name,
		EmojisWithCounts::new(added_emojis),
		group_is_new,
	)
}

pub(super) async fn remove_from_group(
	executor: &Pool<Sqlite>,
	user: UserId,
	emojis: &EmojisWithCounts,
	group: Option<&str>,
) -> EmojisWithCounts {
	let user_id = user.get() as i64;
	let mut degrouped_emojis = Vec::new();

	let mut transaction = executor.begin().await.unwrap();

	for (emoji, count) in emojis {
		let emoji_str = emoji.as_str();
		let rows_affected = if let Some(group) = group {
			query!(
				"
				UPDATE emoji_inventory
				SET group_id = NULL
				WHERE user = ? AND emoji = ? AND emoji_inventory.group_id IS NOT NULL AND rowid IN (
					SELECT emoji_inventory.rowid
					FROM emoji_inventory
					LEFT JOIN emoji_inventory_groups
					ON emoji_inventory.group_id = emoji_inventory_groups.id
					WHERE emoji_inventory.user = ? AND emoji_inventory.emoji = ? AND emoji_inventory_groups.name = ?
					LIMIT ?
				)
				",
				user_id,
				emoji_str,
				user_id,
				emoji_str,
				group,
				count
			)
			.execute(&mut *transaction)
			.await
			.unwrap()
			.rows_affected()
		} else {
			query!(
				"
				UPDATE emoji_inventory
				SET group_id = NULL
				WHERE user = ? AND emoji = ? AND rowid IN (
					SELECT emoji_inventory.rowid
					FROM emoji_inventory
					LEFT JOIN emoji_inventory_groups
					ON emoji_inventory.group_id = emoji_inventory_groups.id
					WHERE emoji_inventory.user = ? AND emoji_inventory.emoji = ?
					ORDER BY IFNULL(sort_order, 9223372036854775807) DESC
					LIMIT ?
				)
				",
				user_id,
				emoji_str,
				user_id,
				emoji_str,
				count
			)
			.execute(&mut *transaction)
			.await
			.unwrap()
			.rows_affected()
		};
		if rows_affected > 0 {
			degrouped_emojis.push((*emoji, rows_affected as u32));
		}
	}

	remove_empty_groups(&mut transaction, user).await;

	transaction.commit().await.unwrap();

	EmojisWithCounts::new(degrouped_emojis)
}

async fn get_current_group_name<'a, E: SqliteExecutor<'a>>(
	executor: E,
	user: UserId,
	group: &str,
) -> Option<String> {
	let user_id = user.get() as i64;
	query!(
		"
		SELECT name
		FROM emoji_inventory_groups
		WHERE user = ? AND name = ?
		",
		user_id,
		group
	)
	.fetch_optional(executor)
	.await
	.unwrap()
	.map(|record| record.name)
}

pub(super) enum RenameGroupError {
	NoSuchGroup,
	NameTaken(String),
}

pub(super) async fn rename_group(
	executor: &Pool<Sqlite>,
	user: UserId,
	group: &str,
	new_name: &str,
) -> Result<String, RenameGroupError> {
	let mut transaction = executor.begin().await.unwrap();
	let Some(old_name) = get_current_group_name(&mut *transaction, user, group).await else {
		return Err(RenameGroupError::NoSuchGroup);
	};

	let user_id = user.get() as i64;
	if let Err(error) = query!(
		"
		UPDATE emoji_inventory_groups
		SET name = ?
		WHERE user = ? AND name = ?
		",
		new_name,
		user_id,
		group
	)
	.execute(&mut *transaction)
	.await
	{
		if matches!(
			error
				.as_database_error()
				.map(|err| err.is_unique_violation()),
			Some(true)
		) {
			let taken_name = get_current_group_name(&mut *transaction, user, new_name)
				.await
				.unwrap();
			return Err(RenameGroupError::NameTaken(taken_name));
		}
		panic!();
	};
	transaction.commit().await.unwrap();

	Ok(old_name)
}

/// Returns group names with their emoji counts, and uncategorized emoji count.
pub(super) async fn list_groups(
	executor: &Pool<Sqlite>,
	user: UserId,
) -> (Vec<(String, u32)>, u32) {
	let user_id = user.get() as i64;
	let records = query!(
		"
		SELECT emoji_inventory_groups.name, COUNT(*) as emoji_count
		FROM emoji_inventory
		LEFT JOIN emoji_inventory_groups
		ON emoji_inventory_groups.id = emoji_inventory.group_id
		WHERE emoji_inventory.user = ?
		GROUP BY emoji_inventory_groups.name, emoji_inventory_groups.user
		ORDER BY emoji_inventory_groups.sort_order ASC
		",
		user_id
	)
	.fetch_all(executor)
	.await
	.unwrap();

	let mut ungrouped = 0;

	let groups = records
		.into_iter()
		.filter_map(|record| match record.name {
			Some(name) => Some((name, record.emoji_count as u32)),
			None => {
				ungrouped = record.emoji_count as u32;
				None
			}
		})
		.collect();

	(groups, ungrouped)
}

/// As empty groups shouldn't be able to exist, an empty result set can be treated as a missing group.
pub(crate) async fn get_group_contents<'a, E: SqliteExecutor<'a>>(
	executor: E,
	emoji_map: &EmojiMap,
	user: UserId,
	group: &str,
) -> EmojisWithCounts {
	let user_id = user.get() as i64;
	let records = query!(
		"
		SELECT emoji, COUNT(*) as count
		FROM emoji_inventory
		LEFT JOIN emoji_inventory_groups
		ON emoji_inventory.group_id = emoji_inventory_groups.id
		WHERE emoji_inventory.user = ? AND emoji_inventory_groups.name = ?
		GROUP BY emoji
		",
		user_id,
		group
	)
	.fetch_all(executor)
	.await
	.unwrap();
	EmojisWithCounts::from_iter(records.into_iter().map(|record| {
		(
			emoji_map.get(record.emoji.as_str()).unwrap(),
			record.count as u32,
		)
	}))
}

pub(super) async fn group_name_and_contents(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	user: UserId,
	group: &str,
) -> Option<(String, EmojisWithCounts)> {
	let mut transaction = database.begin().await.unwrap();

	let name = get_current_group_name(&mut *transaction, user, group).await?;
	let emojis = get_group_contents(&mut *transaction, emoji_map, user, &name).await;

	transaction.commit().await.unwrap();

	Some((name, emojis))
}

pub(super) async fn get_ungrouped_emojis(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	user: UserId,
) -> EmojisWithCounts {
	let user_id = user.get() as i64;
	let records = query!(
		"
		SELECT emoji, COUNT(*) as count
		FROM emoji_inventory
		WHERE user = ? AND group_id IS NULL
		GROUP BY emoji
		",
		user_id,
	)
	.fetch_all(database)
	.await
	.unwrap();
	EmojisWithCounts::from_iter(records.into_iter().map(|record| {
		(
			emoji_map.get(record.emoji.as_str()).unwrap(),
			record.count as u32,
		)
	}))
}

pub(super) enum RepositionOutcome {
	MovedToFront,
	MovedToBack,
	MovedBetween([String; 2]),
	DidNotMove,
}

/// Returns Err if no such group existed, otherwise Ok(name, old_position, group_count).
///
/// Note: in Rust code and in user commands, the ordering starts with 0, but in the database it starts with 1.
pub(super) async fn reposition_group(
	database: &Pool<Sqlite>,
	user: UserId,
	group: &str,
	new_position: u32,
) -> Result<(String, RepositionOutcome, u32, u32), ()> {
	let user_id = user.get() as i64;

	let mut transaction = database.begin().await.unwrap();

	let Some(name) = get_current_group_name(&mut *transaction, user, group).await else {
		return Err(());
	};
	let group = query!(
		"
		SELECT name, sort_order - 1 AS sort_order
		FROM emoji_inventory_groups
		WHERE user = ? AND name  = ?
		",
		user_id,
		name
	)
	.fetch_one(&mut *transaction)
	.await
	.unwrap();

	let name = group.name;
	let current_position = group.sort_order as u32;

	let group_count = query!(
		"
		SELECT COUNT(*) as count
		FROM emoji_inventory_groups
		WHERE user = ?
		",
		user_id
	)
	.fetch_one(&mut *transaction)
	.await
	.unwrap()
	.count as u32;

	let new_position = new_position.min(group_count - 1);

	let scoot_offset = match u32::cmp(&new_position, &current_position) {
		Ordering::Equal => {
			transaction.commit().await.unwrap();
			return Ok((
				name,
				RepositionOutcome::DidNotMove,
				current_position,
				group_count,
			));
		}
		Ordering::Greater => -1,
		Ordering::Less => 1,
	};

	let lower = current_position.min(new_position) + 1;
	let upper = current_position.max(new_position) + 1;
	query!(
		"
		UPDATE emoji_inventory_groups
		SET sort_order = -sort_order
		WHERE user = ? AND sort_order >= ? AND sort_order <= ?;

		UPDATE emoji_inventory_groups
		SET sort_order = ? + 1
		WHERE user = ? AND name = ?;

		UPDATE emoji_inventory_groups
		SET sort_order = -sort_order + ?
		WHERE user = ? AND sort_order >= -? AND sort_order <= -?;
		",
		user_id,
		lower,
		upper,
		new_position,
		user_id,
		name,
		scoot_offset,
		user_id,
		upper,
		lower,
	)
	.execute(&mut *transaction)
	.await
	.unwrap();

	let outcome = if new_position == 0 {
		RepositionOutcome::MovedToFront
	} else if new_position == group_count - 1 {
		RepositionOutcome::MovedToBack
	} else {
		let before = new_position - 1;
		let after = new_position + 1;
		let mut neighbours = query!(
			"
			SELECT name
			FROM emoji_inventory_groups
			WHERE (sort_order IN (? + 1, ? + 1)) AND user = ?
			ORDER BY sort_order ASC
			",
			before,
			after,
			user_id
		)
		.fetch_all(&mut *transaction)
		.await
		.unwrap();
		let neighbour_after = neighbours.pop().unwrap().name;
		let neighbour_before = neighbours.pop().unwrap().name;
		RepositionOutcome::MovedBetween([neighbour_before, neighbour_after])
	};

	transaction.commit().await.unwrap();

	Ok((name, outcome, current_position, group_count))
}
