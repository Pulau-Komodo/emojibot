use crate::{
	emoji::{EmojiMap, EmojiWithImage},
	emojis_with_counts::EmojisWithCounts,
};

/// The base size (in pixels across) of an emoji rendered based on a single inventory emoji.
const EMOJI_SIZE: f32 = 22.0;

const CANVAS_WIDTH: u32 = 500;

#[derive(Debug, Default)]
struct Cursor {
	x: f32,
	y: f32,
}

impl Cursor {
	pub fn new_line(&mut self) {
		self.x = 0.0;
		self.y += EMOJI_SIZE;
	}
	pub fn new_line_if_not_at_start(&mut self) {
		if self.x > 0.0 {
			self.new_line();
		}
	}
	pub fn next(&mut self) {
		self.x += EMOJI_SIZE;
		if self.x > CANVAS_WIDTH as f32 - EMOJI_SIZE {
			self.new_line();
		}
	}
	pub fn to_transform(&self, emoji: EmojiWithImage) -> resvg::tiny_skia::Transform {
		let scale = EMOJI_SIZE / emoji.image().view_box().rect.width();
		resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(self.x, self.y)
	}
}

fn generate(groups: Vec<(String, Vec<EmojiWithImage>)>) -> resvg::tiny_skia::Pixmap {
	let height = groups
		.iter()
		.map(|group| (group.1.len() as f32 * EMOJI_SIZE / 500.0).ceil())
		.sum::<f32>()
		* EMOJI_SIZE;

	let mut canvas =
		resvg::tiny_skia::Pixmap::new(CANVAS_WIDTH, u32::max(height as u32, 1)).unwrap();
	let mut cursor = Cursor::default();

	for (_name, emojis) in groups {
		for emoji in emojis {
			emoji.render(cursor.to_transform(emoji), &mut canvas.as_mut());
			cursor.next();
		}
		cursor.new_line_if_not_at_start();
	}

	canvas
}

fn attach_images(
	emotes: Vec<EmojisWithCounts>,
	emoji_map: &EmojiMap,
) -> Vec<(String, Vec<EmojiWithImage>)> {
	emotes
		.into_iter()
		.map(|emojis| {
			let emojis = emojis
				.into_iter()
				.flat_map(|(emoji, count)| {
					[emoji_map.get_image(emoji)]
						.into_iter()
						.cycle()
						.take(count as usize)
				})
				.collect();
			let name = String::new();
			(name, emojis)
		})
		.collect()
}

pub fn make_inventory_image(emotes: Vec<EmojisWithCounts>, emoji_map: &EmojiMap) -> Vec<u8> {
	let groups = attach_images(emotes, emoji_map);
	let canvas = generate(groups);
	let image = canvas.encode_png().unwrap();
	image
}
