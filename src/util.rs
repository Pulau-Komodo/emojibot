use std::{borrow::Cow, sync::Arc};

use serenity::{
	all::{CommandDataOption, CommandInteraction},
	async_trait,
	builder::{CreateAttachment, CreateInteractionResponse, CreateInteractionResponseMessage},
	http::Http,
	Result as SerenityResult,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
	emoji::{Emoji, EmojiMap},
	special_characters::ZWNJ,
};

#[async_trait]
pub trait ReplyShortcuts {
	async fn reply<S>(&self, http: &Arc<Http>, content: S, ephemeral: bool) -> SerenityResult<()>
	where
		S: Into<String> + Send;
	async fn ephemeral_reply<S>(&self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Into<String> + std::marker::Send;
	async fn public_reply<S>(&self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Into<String> + std::marker::Send;
	async fn reply_image(
		&self,
		http: &Arc<Http>,
		image: &[u8],
		file_name: &str,
	) -> SerenityResult<()>;
}

#[async_trait]
impl ReplyShortcuts for CommandInteraction {
	async fn reply<S>(&self, http: &Arc<Http>, content: S, ephemeral: bool) -> SerenityResult<()>
	where
		S: Into<String> + Send,
	{
		self.create_response(
			http,
			CreateInteractionResponse::Message(
				CreateInteractionResponseMessage::new()
					.content(content)
					.ephemeral(ephemeral),
			),
		)
		.await
	}
	async fn ephemeral_reply<S>(&self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Into<String> + Send,
	{
		self.reply(http, content, true).await
	}
	async fn public_reply<S>(&self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Into<String> + Send,
	{
		self.reply(http, content, false).await
	}
	async fn reply_image(
		&self,
		http: &Arc<Http>,
		image: &[u8],
		file_name: &str,
	) -> SerenityResult<()> {
		self.create_response(
			http,
			CreateInteractionResponse::Message(
				CreateInteractionResponseMessage::new()
					.add_file(CreateAttachment::bytes(image, file_name)),
			),
		)
		.await
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
				.map(|emoji| emoji.emoji());
			Some(emoji)
		})
		.collect()
}

/// Gets the emojis from a specified option index and ensures there is at least one emoji, otherwise returns a user-friendly error string.
pub fn get_and_parse_emoji_option(
	emoji_map: &EmojiMap,
	option: Option<&CommandDataOption>,
) -> Result<Vec<Emoji>, Cow<'static, str>> {
	let input = option
		.and_then(|option| option.value.as_str())
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
