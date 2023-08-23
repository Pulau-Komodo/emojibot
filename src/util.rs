use std::{borrow::Cow, fmt::Display, sync::Arc};

use serenity::{
	async_trait,
	http::Http,
	model::prelude::{
		application_command::{ApplicationCommandInteraction, CommandDataOption},
		InteractionResponseType,
	},
	Result as SerenityResult,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
	emoji::{Emoji, EmojiMap},
	special_characters::ZWNJ,
};

#[async_trait]
pub trait ReplyShortcuts {
	async fn reply<S>(self, http: &Arc<Http>, content: S, ephemeral: bool) -> SerenityResult<()>
	where
		S: Display + std::marker::Send;
	async fn ephemeral_reply<S>(self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Display + std::marker::Send;
	async fn public_reply<S>(self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Display + std::marker::Send;
}

#[async_trait]
impl ReplyShortcuts for ApplicationCommandInteraction {
	async fn reply<S>(self, http: &Arc<Http>, content: S, ephemeral: bool) -> SerenityResult<()>
	where
		S: Display + Send,
	{
		self.create_interaction_response(http, |response| {
			response
				.kind(InteractionResponseType::ChannelMessageWithSource)
				.interaction_response_data(|message| message.content(content).ephemeral(ephemeral))
		})
		.await
	}
	async fn ephemeral_reply<S>(self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Display + Send,
	{
		self.reply(http, content, true).await
	}
	async fn public_reply<S>(self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Display + Send,
	{
		self.reply(http, content, false).await
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
