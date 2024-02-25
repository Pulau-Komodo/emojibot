//! Module for the commands that generate a randomized image with emojis.

use std::f32::consts::PI;

use rand::Rng;
use rand_distr::Distribution;
use resvg::usvg::TreeParsing;
use serenity::{
	all::{CommandInteraction, CommandOptionType, UserId},
	builder::{CreateCommand, CreateCommandOption},
	model::Permissions,
};
use sqlx::{Pool, Sqlite};

use crate::{
	context::Context,
	emoji::{Emoji, EmojiMap},
	emojis_with_counts::EmojisWithCounts,
	inventory::queries::get_group_contents,
	util::{parse_emoji_input, ReplyShortcuts},
};

use super::read_emoji_svg;

/// The base size (in pixels across) of an emoji rendered based on a single inventory emoji.
const EMOJI_SIZE: f32 = 90.0;

const CANVAS_WIDTH: u32 = 500;
const CANVAS_HEIGHT: u32 = 250;
const EMOJI_REPETITION: usize = 5;

fn random_angle(rng: &mut rand::rngs::ThreadRng) -> f32 {
	rand_distr::Normal::new(0.0, 0.125 * PI)
		.unwrap()
		.sample(rng)
}

struct EmojiToRender {
	emoji: Emoji,
	/// Size in multiple of base size.
	size: f32,
}

impl EmojiToRender {
	fn new(emoji: Emoji, fraction: f32) -> Self {
		Self {
			emoji,
			size: fraction.sqrt(),
		}
	}
}

fn place_emoji_randomly(
	canvas: &mut resvg::tiny_skia::PixmapMut,
	tree: &resvg::Tree,
	size: f32,
	rng: &mut rand::rngs::ThreadRng,
) {
	let canvas_width = canvas.width() as f32;
	let canvas_height = canvas.height() as f32;
	let size = size * EMOJI_SIZE;
	let size_with_margin = (size.powi(2) * 2.0).sqrt().ceil();
	let half_margin = ((size_with_margin - size) / 2.0).ceil();
	// Add half rotation margin so rotation can't make it go over the left or top edges.
	let x = rng.gen_range(0.0..canvas_width) + half_margin;
	let y = rng.gen_range(0.0..canvas_height) + half_margin;

	let scale = size / tree.view_box.rect.width();
	let angle = random_angle(rng).to_degrees();
	let transform = resvg::tiny_skia::Transform::from_rotate_at(
		angle,
		tree.view_box.rect.width() / 2.0,
		tree.view_box.rect.height() / 2.0,
	)
	.post_scale(scale, scale);

	tree.render(transform.post_translate(x, y), canvas);

	if x + size_with_margin > canvas_width {
		let x = x - canvas_width;
		tree.render(transform.post_translate(x, y), canvas);
	}
	if y + size_with_margin > canvas_height {
		let y = y - canvas_height;
		tree.render(transform.post_translate(x, y), canvas);
	}
	if x + size_with_margin > canvas_width && y + size_with_margin > canvas_height {
		let x = x - canvas_width;
		let y = y - canvas_height;
		tree.render(transform.post_translate(x, y), canvas);
	}
}

async fn parse_emoji_and_group_input<'s>(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	user: UserId,
	input: &'s str,
) -> Result<EmojisWithCounts, String> {
	let mut emojis = Vec::new();
	for substring in input.split(',') {
		let substring = substring.trim();
		if let Ok(parsed_emojis) = parse_emoji_input(emoji_map, substring) {
			emojis.extend(parsed_emojis);
		} else {
			let group_emojis = get_group_contents(database, emoji_map, user, substring)
				.await
				.flatten();
			if group_emojis.is_empty() {
				return Err(format!("You do not have a group named \"{substring}\"."));
			}
			emojis.extend(group_emojis);
		}
	}
	Ok(EmojisWithCounts::from_flat(&emojis))
}

fn generate(emojis: impl IntoIterator<Item = EmojiToRender>) -> Option<resvg::tiny_skia::Pixmap> {
	let mut canvas = resvg::tiny_skia::Pixmap::new(CANVAS_WIDTH, CANVAS_HEIGHT).unwrap();
	let image_trees = emojis
		.into_iter()
		.map(|emoji| {
			let svg = read_emoji_svg(&emoji.emoji)?;
			let tree =
				resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default()).unwrap();
			Some((resvg::Tree::from_usvg(&tree), emoji.size))
		})
		.collect::<Option<Vec<_>>>()?;

	let mut rng = rand::thread_rng();
	let canvas_mut = &mut canvas.as_mut();
	for _ in 0..EMOJI_REPETITION {
		for (tree, size) in &image_trees {
			place_emoji_randomly(canvas_mut, tree, *size, &mut rng);
		}
	}
	Some(canvas)
}

pub async fn execute_test(context: Context<'_>, interaction: CommandInteraction) {
	let _ = interaction
		.public_reply(context.http, "No test currently active.")
		.await;
}

pub fn register_test() -> CreateCommand {
	CreateCommand::new("testimage")
		.description("IDK")
		.default_member_permissions(Permissions::ADMINISTRATOR)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"emoji",
				"The emoji to wibbly wobble.",
			)
			.required(true),
		)
}

pub async fn execute(context: Context<'_>, interaction: CommandInteraction) {
	let input = interaction
		.data
		.options
		.first()
		.and_then(|option| option.value.as_str())
		.unwrap();
	let emojis = match parse_emoji_and_group_input(
		context.database,
		context.emoji_map,
		interaction.user.id,
		input,
	)
	.await
	{
		Ok(emojis) => emojis,
		Err(message) => {
			let _ = interaction.ephemeral_reply(context.http, message).await;
			return;
		}
	};
	if !emojis
		.are_owned_by_user(context.database, interaction.user.id)
		.await
	{
		let _ = interaction
			.ephemeral_reply(context.http, "You don't own all specified emojis.")
			.await;
		return;
	}

	let Some(canvas) = generate(
		emojis
			.flatten()
			.into_iter()
			.map(|emoji| EmojiToRender::new(emoji, 0.2)),
	) else {
		let _ = interaction
			.ephemeral_reply(context.http, "Some file missing.")
			.await;
		return;
	};
	let image = canvas.encode_png().unwrap();

	let _ = interaction
		.reply_image(context.http, image.as_slice(), "image.png")
		.await;
}

pub fn register() -> CreateCommand {
	CreateCommand::new("generate")
		.description("Generage an image using your emojis.")
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"emojis",
				"The emojis to use. You can use emojis and emoji groups together, comma-separated.",
			)
			.required(true),
		)
}
