use std::{borrow::Cow, ops::Range, str::FromStr, sync::Arc};

use serenity::{
	all::{CommandDataOption, CommandInteraction},
	async_trait,
	builder::{
		CreateAttachment, CreateInteractionResponse, CreateInteractionResponseFollowup,
		CreateInteractionResponseMessage,
	},
	http::Http,
	Result as SerenityResult,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
	emoji::{Emoji, EmojiMap},
	images::generate::EmojiToRender,
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
	async fn follow_up_image(
		&self,
		http: &Arc<Http>,
		image: &[u8],
		file_name: &str,
	) -> SerenityResult<serenity::all::Message>;
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
	async fn follow_up_image(
		&self,
		http: &Arc<Http>,
		image: &[u8],
		file_name: &str,
	) -> SerenityResult<serenity::all::Message> {
		self.create_followup(
			http,
			CreateInteractionResponseFollowup::new()
				.add_file(CreateAttachment::bytes(image, file_name)),
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
			let emoji = emoji_map.get(grapheme).ok_or_else(|| {
				format!("Could not recognize \"{grapheme}\" as an emoji in my list.")
			});
			Some(emoji)
		})
		.collect()
}

pub fn parse_emoji_input_with_modifiers<'l>(
	emoji_map: &'l EmojiMap,
	input: &str,
) -> Result<Vec<EmojiToRender<'l>>, String> {
	const DEFAULT_EMOJI_SIZE: f32 = 1.0;
	const DEFAULT_EMOJI_COUNT: usize = 1;
	let mut emojis = Vec::new();
	let mut last_emoji = None;
	let mut size_modifier: Option<Range<usize>> = None;
	let mut multiplier: Option<Range<usize>> = None;
	for (index, mut grapheme) in input.grapheme_indices(true) {
		if grapheme == " " || grapheme == ZWNJ {
			continue;
		}
		grapheme = grapheme.trim_end_matches(ZWNJ);
		if let Some(emoji) = emoji_map.get_with_image(grapheme) {
			if let Some(emoji) = last_emoji.take() {
				let size: f32 = consume_number_input(input, &mut size_modifier, DEFAULT_EMOJI_SIZE)
					.map_err(|err| format!("Error parsing {} as size modifier", err))?;
				let multiplier: usize =
					consume_number_input(input, &mut multiplier, DEFAULT_EMOJI_COUNT)
						.map_err(|err| format!("Error parsing {} as multiplier", err))?;
				emojis.push(EmojiToRender::new(emoji, size, multiplier));
			} else if size_modifier.is_some() || multiplier.is_some() {
				return Err(String::from("Error parsing input."));
			}
			last_emoji = Some(emoji);
		} else {
			let end_index = index + grapheme.len();
			if let Some(ref mut multiplier) = &mut multiplier.as_mut() {
				multiplier.end = end_index;
			} else if grapheme == "x" || grapheme == "*" {
				multiplier = Some(end_index..end_index);
			} else if let Some(ref mut size_modifier) = &mut size_modifier {
				size_modifier.end = end_index;
			} else {
				size_modifier = Some(index..end_index);
			}
		}
	}
	if let Some(emoji) = last_emoji.take() {
		let size: f32 = consume_number_input(input, &mut size_modifier, DEFAULT_EMOJI_SIZE)
			.map_err(|err| format!("Error parsing {} as size modifier", err))?;
		let multiplier: usize = consume_number_input(input, &mut multiplier, DEFAULT_EMOJI_COUNT)
			.map_err(|err| format!("Error parsing {} as multiplier", err))?;
		emojis.push(EmojiToRender::new(emoji, size, multiplier));
	} else if size_modifier.is_some() || multiplier.is_some() {
		return Err(String::from("Error parsing input."));
	}
	Ok(emojis)
}

fn consume_number_input<'l, T>(
	input: &'l str,
	range: &mut Option<Range<usize>>,
	default: T,
) -> Result<T, &'l str>
where
	T: FromStr,
{
	Ok(range
		.take()
		.map(|range| {
			let text = &input[range];
			text.parse().map_err(|_| text)
		})
		.transpose()?
		.unwrap_or(default))
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
