use std::{collections::HashMap, fmt::Display};

use serenity::model::prelude::UserId;
use sqlx::{query, Pool, Sqlite};

use crate::{
	emoji::{Emoji, EmojiMap},
	special_characters::ZWNJ,
};

/// A list of emojis with a count for each emoji, sorted by emoji.
#[derive(PartialEq, Eq, Clone)]
pub struct EmojisWithCounts(Vec<(Emoji, u32)>);

impl EmojisWithCounts {
	pub fn new(mut emojis: Vec<(Emoji, u32)>) -> Self {
		emojis.sort_unstable();
		Self(emojis)
	}
	pub fn from_iter(emojis: impl IntoIterator<Item = (Emoji, u32)>) -> Self {
		let mut emojis = emojis.into_iter().collect::<Vec<_>>();
		emojis.sort_unstable();
		Self(emojis)
	}
	pub async fn from_database_for_user(
		executor: &Pool<Sqlite>,
		emoji_map: &EmojiMap,
		user: UserId,
	) -> Self {
		let user_id = user.get() as i64;
		let mut emojis = query!(
			"
			SELECT emoji, COUNT(*) AS count
			FROM emoji_inventory
			WHERE user = ?
			GROUP BY emoji
			",
			user_id
		)
		.fetch_all(executor)
		.await
		.unwrap()
		.into_iter()
		.filter_map(|record| {
			(record.count > 0).then_some((
				*emoji_map
					.get(record.emoji.as_str())
					.expect("Emoji from database was somehow not in map."),
				record.count as u32,
			))
		})
		.collect::<Vec<_>>();
		emojis.sort_unstable();
		Self(emojis)
	}
	pub fn from_flat<'l>(iter: impl IntoIterator<Item = &'l Emoji>) -> Self {
		let mut emojis = HashMap::new();

		for &emoji in iter {
			*emojis.entry(emoji).or_insert(0) += 1;
		}

		let mut emojis = emojis.into_iter().collect::<Vec<_>>();
		emojis.sort_unstable();
		Self(emojis)
	}
	pub fn flatten(self) -> Vec<Emoji> {
		self.0
			.into_iter()
			.flat_map(|(emoji, count)| [emoji].into_iter().cycle().take(count as usize))
			.collect()
	}

	/// Check a user's emoji inventory to see if it has the emojis.
	pub async fn are_owned_by_user(&self, database: &Pool<Sqlite>, user: UserId) -> bool {
		let user_id = user.get() as i64;
		let mut transaction = database.begin().await.unwrap();
		for (emoji, target_count) in &self.0 {
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
			if (count as u32) < *target_count {
				transaction.commit().await.unwrap();
				return false;
			}
		}
		transaction.commit().await.unwrap();
		true
	}

	pub fn iter(&self) -> std::slice::Iter<(Emoji, u32)> {
		self.0.iter()
	}

	/// The number of different emojis.
	pub fn unique_emoji_count(&self) -> usize {
		self.0.len()
	}

	/// The number of emojis, counting duplicates.
	pub fn emoji_count(&self) -> u32 {
		self.0.iter().fold(0, |sum, (_, count)| sum + count)
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}
}

impl Display for EmojisWithCounts {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for (emoji, count) in &self.0 {
			f.write_str(emoji.as_str())?;
			if *count > 1 {
				f.write_fmt(format_args!("x{count}"))?;
			} else if (1580..=1605).contains(&emoji.index()) {
				f.write_str(ZWNJ)?; // To avoid regional indicator emojis combining inappropriately.
			}
		}
		Ok(())
	}
}

impl IntoIterator for EmojisWithCounts {
	type Item = (Emoji, u32);

	type IntoIter = std::vec::IntoIter<Self::Item>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.into_iter()
	}
}

impl<'l> IntoIterator for &'l EmojisWithCounts {
	type Item = &'l (Emoji, u32);

	type IntoIter = std::slice::Iter<'l, (Emoji, u32)>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}
