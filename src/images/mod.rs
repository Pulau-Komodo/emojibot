//! Module for image generation.

use crate::emoji::Emoji;

pub mod generate;
pub mod rasterize;

fn read_emoji_svg(emoji: &Emoji) -> Option<Vec<u8>> {
	let path = std::path::PathBuf::from(format!("./assets/svg/{}", emoji.file_name()));
	match std::fs::read(path) {
		Ok(data) => Some(data),
		Err(error) => {
			eprintln!("{}", error);
			eprintln!(
				"\"{}\" was not found in the emoji .svg files.",
				emoji.as_str()
			);
			None
		}
	}
}
