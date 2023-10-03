use std::{
	collections::HashMap,
	fmt::{Display, Write},
};

use crate::emoji_list::EMOJI_LIST;
use rand::{thread_rng, Rng};
use serenity::model::prelude::ReactionType;

const VS16: char = '\u{fe0f}';

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
		let is_short = self.emoji.chars().nth(3).is_none();
		for char in self.emoji.chars() {
			if is_short && char == VS16 {
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

pub type EmojiMap = HashMap<&'static str, Emoji>;

pub fn make_emoji_map() -> EmojiMap {
	EMOJI_LIST
		.into_iter()
		.enumerate()
		.map(|(index, emoji)| (emoji, Emoji { emoji, index }))
		.collect()
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
