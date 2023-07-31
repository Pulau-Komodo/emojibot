use std::{
	collections::HashMap,
	fmt::{Display, Write},
};

use rand::{thread_rng, Rng};
use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{application_command::ApplicationCommandInteraction, ReactionType, UserId},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{emoji_list::EMOJI_LIST, util::interaction_reply};

const VS16: char = '\u{fe0f}';

pub type EmojiMap = HashMap<&'static str, Emoji>;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct Emoji {
	emoji: &'static str,
	index: usize,
}

impl Emoji {
	pub fn random() -> Self {
		let index = thread_rng().gen_range(0..EMOJI_LIST.len());
		Self {
			emoji: EMOJI_LIST[index],
			index,
		}
	}
	pub fn as_str(self) -> &'static str {
		self.emoji
	}
	/// Get the file name for the Twemoji .svg file for this emoji, like "1f642.svg".
	pub fn file_name(&self) -> String {
		let mut string = String::with_capacity(self.emoji.len() * 6 + 3);
		for (index, char) in self.emoji.chars().enumerate() {
			if char == VS16 && index < 3 {
				// For some reason, Twemoji file names never include VS16 on shorter emojis, even though some of them should have it.
				continue;
			}
			if !string.is_empty() {
				string.push('-');
			}
			write!(string, "{:x}", char as u32).unwrap();
		}
		string + ".svg"
	}
}

impl Display for Emoji {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl PartialOrd for Emoji {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.index.partial_cmp(&other.index)
	}
}

impl Ord for Emoji {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.index.cmp(&other.index)
	}
}

impl From<Emoji> for ReactionType {
	fn from(emoji: Emoji) -> ReactionType {
		ReactionType::Unicode(String::from(emoji.as_str()))
	}
}

pub fn make_emoji_map() -> EmojiMap {
	EMOJI_LIST
		.into_iter()
		.enumerate()
		.map(|(index, emoji)| (emoji, Emoji { emoji, index }))
		.collect()
}

async fn get_user_emojis(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	user: UserId,
) -> Vec<(Emoji, u32)> {
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
			record.count as u32,
		))
	})
	.collect::<Vec<_>>();
	emojis.sort_unstable();
	emojis
}

pub async fn command_list_emojis(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: Context,
	interaction: ApplicationCommandInteraction,
) {
	let emojis = get_user_emojis(database, emoji_map, interaction.user.id).await;
	if emojis.is_empty() {
		interaction_reply(context, interaction, "You have no emojis. 🤔", false)
			.await
			.unwrap();
		return;
	}
	let mut output = if emojis.len() == 1 {
		String::from("You only have ")
	} else {
		String::from("You have the following emojis: ")
	};
	for (emoji, count) in emojis {
		output.push_str(emoji.as_str());
		if count > 1 {
			write!(output, "x{count}").unwrap();
		}
	}
	output.push('.');
	interaction_reply(context, interaction, output, false)
		.await
		.unwrap();
}

pub fn register_list_emojis(
	command: &mut CreateApplicationCommand,
) -> &mut CreateApplicationCommand {
	command
		.name("emojis")
		.description("Check your emoji inventory.")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn emoji_file_name() {
		let emoji = Emoji {
			emoji: "🙂",
			index: 0,
		};
		assert_eq!(emoji.file_name(), "1f642.svg"); // Basic
		let emoji = Emoji {
			emoji: "👍🏽",
			index: 0,
		};
		assert_eq!(emoji.file_name(), "1f44d-1f3fd.svg"); // Skin tone modifier
		let emoji = Emoji {
			emoji: "💇‍♂️",
			index: 0,
		};
		assert_eq!(emoji.file_name(), "1f487-200d-2642-fe0f.svg"); // ZWJ, male modifier, variant selector
		let emoji = Emoji {
			emoji: "👨‍👩‍👧‍👦",
			index: 0,
		};
		assert_eq!(
			emoji.file_name(),
			"1f468-200d-1f469-200d-1f467-200d-1f466.svg"
		); // Large ZWJ-based composite
	}
}
