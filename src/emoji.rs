use std::{
	collections::HashMap,
	fmt::{Display, Write},
	hash::Hash,
};

use crate::emoji_list::EMOJI_LIST;
use rand::{thread_rng, Rng};
use serenity::model::prelude::ReactionType;

const VS16: char = '\u{fe0f}';

#[derive(Debug, Clone, Copy)]
pub struct Emoji {
	emoji: &'static str,
	/// The position in the original emoji array (which is ordered).
	index: usize,
}

impl Display for Emoji {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.emoji)
	}
}

impl PartialOrd for Emoji {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.index.cmp(&other.index))
	}
}

impl Ord for Emoji {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.index.cmp(&other.index)
	}
}

impl PartialEq for Emoji {
	fn eq(&self, other: &Self) -> bool {
		self.index == other.index
	}
}

impl Hash for Emoji {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		state.write_usize(self.index);
	}
}

impl Eq for Emoji {}

impl Emoji {
	pub fn random() -> Self {
		let index = thread_rng().gen_range(0..EMOJI_LIST.len());
		Self {
			emoji: EMOJI_LIST[index],
			index,
		}
	}
	pub fn as_str(&self) -> &'static str {
		self.emoji
	}
	pub fn index(&self) -> usize {
		self.index
	}
	fn file_name(self) -> String {
		// 5 characters per byte plus one for each dividing "-" or the "." at the end, plus 3 for "svg".
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
	fn read_svg(self) -> Option<Vec<u8>> {
		let path = std::path::PathBuf::from(format!("./assets/svg/{}", self.file_name()));
		match std::fs::read(path) {
			Ok(data) => Some(data),
			Err(error) => {
				eprintln!("{}", error);
				eprintln!(
					"\"{}\" was not found in the emoji .svg files.",
					self.as_str()
				);
				None
			}
		}
	}
}

#[derive(Debug, Clone)]
pub struct EmojiWithImage {
	emoji: Emoji,
	/// An SVG render tree.
	image: resvg::usvg::Tree,
}

impl EmojiWithImage {
	pub fn emoji(&self) -> Emoji {
		self.emoji
	}
	pub fn str(&self) -> &'static str {
		self.emoji.as_str()
	}
	pub fn index(&self) -> usize {
		self.emoji.index
	}
	pub fn image(&self) -> &resvg::usvg::Tree {
		&self.image
	}
	pub fn render(
		&self,
		transform: resvg::tiny_skia::Transform,
		pixmap: &mut resvg::tiny_skia::PixmapMut,
	) {
		resvg::render(&self.image, transform, pixmap);
	}
}

impl PartialOrd for EmojiWithImage {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.emoji.index.cmp(&other.emoji.index))
	}
}

impl Ord for EmojiWithImage {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.emoji.index.cmp(&other.emoji.index)
	}
}

impl PartialEq for EmojiWithImage {
	fn eq(&self, other: &Self) -> bool {
		self.emoji.index == other.emoji.index
	}
}

impl Eq for EmojiWithImage {}

impl From<EmojiWithImage> for ReactionType {
	fn from(emoji: EmojiWithImage) -> ReactionType {
		ReactionType::Unicode(String::from(emoji.str()))
	}
}

impl From<Emoji> for ReactionType {
	fn from(emoji: Emoji) -> ReactionType {
		ReactionType::Unicode(String::from(emoji.as_str()))
	}
}

pub type EmojiMap = HashMap<&'static str, EmojiWithImage>;

pub fn make_emoji_map() -> EmojiMap {
	let mut emoji_map = EmojiMap::with_capacity(EMOJI_LIST.len());
	let options = resvg::usvg::Options::default();
	let fonts = resvg::usvg::fontdb::Database::default();
	emoji_map.extend(
		EMOJI_LIST
			.into_iter()
			.enumerate()
			.filter_map(|(index, emoji)| {
				let emoji = Emoji { emoji, index };
				let image = emoji.read_svg()?;
				let image = resvg::usvg::Tree::from_data(&image, &options, &fonts).ok()?;
				Some((emoji.as_str(), EmojiWithImage { emoji, image }))
			}),
	);
	emoji_map
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_emoji_file_name() {
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
	#[test]
	fn find_a_and_z() {
		let map = make_emoji_map();
		assert_eq!(
			(Some(1605), Some(1580)),
			(
				map.get(&"🇦").map(EmojiWithImage::index),
				map.get(&"🇿").map(EmojiWithImage::index),
			)
		)
	}
}
