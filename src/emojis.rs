use std::{collections::HashMap, fmt::Display};

use rand::{seq::SliceRandom, thread_rng};
use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{application_command::ApplicationCommandInteraction, ReactionType, UserId},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{emoji_list::EMOJI_LIST, util::interaction_reply};

pub type EmojiMap = HashMap<&'static str, (Emoji, usize)>;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct Emoji(&'static str);

impl Emoji {
	pub fn random() -> Self {
		let emoji = *EMOJI_LIST.choose(&mut thread_rng()).unwrap();
		Self(emoji)
	}
	pub fn inner(self) -> &'static str {
		self.0
	}
}

impl Display for Emoji {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl From<Emoji> for ReactionType {
	fn from(emoji: Emoji) -> ReactionType {
		ReactionType::Unicode(String::from(emoji.0))
	}
}

pub fn make_emoji_map() -> EmojiMap {
	EMOJI_LIST
		.into_iter()
		.enumerate()
		.map(|(index, emoji)| (emoji, (Emoji(emoji), index)))
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
			emoji_map
				.get(record.emoji.as_str())
				.expect("Emoji from database was somehow not in map.")
				.0,
			record.count as u32,
		))
	})
	.collect::<Vec<_>>();
	emojis.sort_unstable_by(|a, b| {
		usize::cmp(
			&emoji_map.get(a.0.inner()).unwrap().1,
			&emoji_map.get(b.0.inner()).unwrap().1,
		)
	});
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
		output.push_str(emoji.inner());
		if count > 1 {
			output.extend(format!("({count})").chars());
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
