use std::{
	cmp::Ordering,
	collections::HashMap,
	fmt::{Display, Write},
	hash::Hash,
	ops::Range,
};

use crate::emoji_list::EMOJI_LIST;
use rand::{thread_rng, Rng};
use serenity::model::prelude::ReactionType;

const VS16: char = '\u{fe0f}';

const NON_MIRROR_EMOJIS: [Range<usize>; 29] = [
	53..54,
	100..101,
	134..136,
	146..148,
	157..159,
	638..649,
	650..653,
	894..897,
	910..911,
	913..914,
	919..920,
	928..929,
	936..937,
	1033..1034,
	1046..1047,
	1072..1074,
	1077..1082,
	1085..1087,
	1109..1114,
	1115..1118,
	1163..1164,
	1173..1174,
	1214..1215,
	1226..1227,
	1233..1235,
	1237..1239,
	1277..1279,
	1304..1538,
	1535..1875,
];

const NON_ROTATE_EMOJIS: [Range<usize>; 7] = [
	130..132,
	134..136,
	146..151,
	641..649,
	1443..1444,
	1448..1471,
	1552..1576,
];

#[derive(Debug, Clone, Copy)]
pub struct Emoji {
	emoji: &'static str,
	/// The position in the original emoji array (which is ordered).
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

impl Display for Emoji {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.emoji)
	}
}

impl PartialOrd for Emoji {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
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

#[derive(Debug, Clone, Copy)]
pub struct EmojiWithImage<'t> {
	emoji: Emoji,
	/// An SVG render tree.
	image: &'t resvg::usvg::Tree,
	/// Whether it's OK to mirror the emoji. Emojis with text should not be mirrored, for example.
	may_mirror: bool,
	/// Whether it's OK to rotate the emoji. Mainly it's just emojis with meaningful directions that should not be rotated.
	may_rotate: bool,
}

impl<'t> EmojiWithImage<'t> {
	fn new(emoji: Emoji, image: &'t resvg::usvg::Tree) -> Self {
		let search_range = |range: &Range<usize>| {
			if range.start > emoji.index() {
				Ordering::Greater
			} else if range.end <= emoji.index() {
				Ordering::Less
			} else {
				Ordering::Equal
			}
		};
		let should_mirror = NON_MIRROR_EMOJIS.binary_search_by(search_range).is_err();
		let should_rotate = NON_ROTATE_EMOJIS.binary_search_by(search_range).is_err();
		Self {
			emoji,
			image,
			may_mirror: should_mirror,
			may_rotate: should_rotate,
		}
	}
	pub fn emoji(&self) -> Emoji {
		self.emoji
	}
	pub fn str(&self) -> &'static str {
		self.emoji.as_str()
	}
	pub fn index(&self) -> usize {
		self.emoji.index
	}
	pub fn image(&'t self) -> &'t resvg::usvg::Tree {
		self.image
	}
	pub fn render(
		&self,
		transform: resvg::tiny_skia::Transform,
		pixmap: &mut resvg::tiny_skia::PixmapMut,
	) {
		resvg::render(self.image, transform, pixmap);
	}
	pub fn may_mirror(&self) -> bool {
		self.may_mirror
	}
	pub fn may_rotate(&self) -> bool {
		self.may_rotate
	}
}

impl PartialOrd for EmojiWithImage<'_> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for EmojiWithImage<'_> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.emoji.index.cmp(&other.emoji.index)
	}
}

impl PartialEq for EmojiWithImage<'_> {
	fn eq(&self, other: &Self) -> bool {
		self.emoji.index == other.emoji.index
	}
}

impl Eq for EmojiWithImage<'_> {}

impl From<EmojiWithImage<'_>> for ReactionType {
	fn from(emoji: EmojiWithImage) -> ReactionType {
		ReactionType::Unicode(String::from(emoji.str()))
	}
}

impl From<Emoji> for ReactionType {
	fn from(emoji: Emoji) -> ReactionType {
		ReactionType::Unicode(String::from(emoji.as_str()))
	}
}

pub fn load_emojis() -> Vec<resvg::usvg::Tree> {
	let options = resvg::usvg::Options::default();
	let fonts = resvg::usvg::fontdb::Database::default();
	EMOJI_LIST
		.into_iter()
		.enumerate()
		.map(|(index, emoji)| {
			let emoji = Emoji { emoji, index };
			let image = emoji.read_svg().unwrap();
			resvg::usvg::Tree::from_data(&image, &options, &fonts).unwrap()
		})
		.collect()
}

pub struct EmojiMap {
	map: HashMap<&'static str, Emoji>,
	images: Vec<resvg::usvg::Tree>,
}

impl EmojiMap {
	pub fn load() -> Self {
		println!("Loading images.");
		let images = load_emojis();
		let map = EMOJI_LIST
			.into_iter()
			.enumerate()
			.map(|(index, emoji)| (emoji, Emoji { emoji, index }))
			.collect();
		println!("Images loaded.");
		Self { map, images }
	}
	pub fn get(&self, emoji: &str) -> Option<Emoji> {
		self.map.get(emoji).copied()
	}
	pub fn get_with_image(&'_ self, emoji: &str) -> Option<EmojiWithImage<'_>> {
		self.map.get(emoji).map(|emoji| self.get_image(*emoji))
	}
	/// This gets the image by index, avoiding a hash look-up.
	pub fn get_image(&'_ self, emoji: Emoji) -> EmojiWithImage<'_> {
		let image = &self.images[emoji.index()];
		EmojiWithImage::new(emoji, image)
	}
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
		assert_eq!(EMOJI_LIST[1605], "🇦");
		assert_eq!(EMOJI_LIST[1580], "🇿");
	}
}
