use std::{borrow::Cow, fmt::Display};

use serenity::{
	model::prelude::{
		application_command::{ApplicationCommandInteraction, CommandDataOption},
		GuildId, InteractionResponseType, UserId,
	},
	prelude::Context,
	Result as SerenityResult,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
	emoji::{Emoji, EmojiMap},
	special_characters::ZWNJ,
};

pub async fn interaction_reply<S>(
	context: Context,
	interaction: ApplicationCommandInteraction,
	content: S,
	ephemeral: bool,
) -> SerenityResult<()>
where
	S: Display,
{
	interaction
		.create_interaction_response(&context.http, |response| {
			response
				.kind(InteractionResponseType::ChannelMessageWithSource)
				.interaction_response_data(|message| message.content(content).ephemeral(ephemeral))
		})
		.await
}

pub async fn ephemeral_reply<S>(
	context: Context,
	interaction: ApplicationCommandInteraction,
	content: S,
) -> SerenityResult<()>
where
	S: Display,
{
	interaction_reply(context, interaction, content, true).await
}

pub async fn public_reply<S>(
	context: Context,
	interaction: ApplicationCommandInteraction,
	content: S,
) -> SerenityResult<()>
where
	S: Display,
{
	interaction_reply(context, interaction, content, false).await
}

/// Gives nickname if possible, otherwise display name, otherwise ID as a string.
pub async fn get_name(context: &Context, guild: GuildId, user: UserId) -> String {
	let member = if let Some(member) = context.cache.member(guild, user) {
		member
	} else if let Ok(member) = context.http.get_member(guild.0, user.0).await {
		member
	} else {
		return format!("{}", user.0);
	};
	if let Some(nick) = member.nick {
		nick
	} else {
		member.display_name().to_string()
	}
}

pub fn parse_emoji_input(emoji_map: &EmojiMap, input: &str) -> Result<Vec<Emoji>, String> {
	input
		.graphemes(true)
		.filter_map(|mut grapheme| {
			if grapheme == " " || grapheme == ZWNJ {
				return None;
			}
			grapheme = grapheme.trim_end_matches(ZWNJ);
			let emoji = emoji_map
				.get(grapheme)
				.ok_or_else(|| {
					format!("Could not recognize \"{grapheme}\" as an emoji in my list.")
				})
				.map(|emoji| *emoji);
			Some(emoji)
		})
		.collect()
}

/// Gets the emojis from a specified option index and ensures there is at least one emoji, otherwise returns a user-friendly error string.
pub fn get_and_parse_emoji_option(
	emoji_map: &EmojiMap,
	options: &[CommandDataOption],
	index: usize,
) -> Result<Vec<Emoji>, Cow<'static, str>> {
	let input = options
		.get(index)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.ok_or("Emojis argument not supplied.")?;

	let emojis = parse_emoji_input(emoji_map, input)?;

	if emojis.is_empty() {
		Err("You did not specify any emojis.")?;
	}
	Ok(emojis)
}

#[cfg(test)]
mod tests {
	use super::*;

	/// This is basically just validating my understanding of the grapheme iterator as it relates to ZWNJ.
	#[test]
	fn grapheme_clusters() {
		let messy_input = "\u{200C}👍👍\u{200C}🤔 \u{200C}\u{200C}\u{200C}";
		let expected_outcome = [
			"\u{200C}",
			"👍",
			"👍\u{200C}",
			"🤔",
			" \u{200C}\u{200C}\u{200C}",
		];
		for (input, expected) in messy_input.graphemes(true).zip(expected_outcome) {
			assert_eq!(input, expected);
		}
		let expected_trimmed_outcome = ["", "👍", "👍", "🤔", " "];
		for (input, expected) in messy_input.graphemes(true).zip(expected_trimmed_outcome) {
			assert_eq!(input.trim_end_matches(ZWNJ), expected);
		}
	}
}
