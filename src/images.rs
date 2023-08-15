use std::f32::consts::PI;

use image::RgbaImage;
use rand::Rng;
use rand_distr::Distribution;
use resvg::usvg::TreeParsing;
use serenity::{
	builder::CreateApplicationCommand,
	model::{
		prelude::{
			application_command::ApplicationCommandInteraction, command::CommandOptionType,
			InteractionResponseType, UserId,
		},
		Permissions,
	},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{
	emoji::{Emoji, EmojiMap},
	queries::{does_user_have_emojis, get_user_emojis},
	util::{interaction_reply, parse_emoji_input},
};

const EMOJI_SIZE: u32 = 48;
const EMOJI_SUPERSAMPLED_SIZE: u32 = EMOJI_SIZE * 2;
const EMOJI_SIZE_WITH_ROTATION_MARGIN: u32 = 136;

async fn has_emoji(executor: &Pool<Sqlite>, emoji: Emoji, user: UserId) -> bool {
	let user_id = user.0 as i64;
	let emoji_str = emoji.as_str();
	query!(
		"
		SELECT
			count
		FROM
			emoji_inventory
		WHERE
			user = ? AND emoji = ?
		",
		user_id,
		emoji_str
	)
	.fetch_optional(executor)
	.await
	.unwrap()
	.map(|record| record.count > 0)
	.unwrap_or(false)
}

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

fn pixmap_to_rgba_image(pixmap: resvg::tiny_skia::Pixmap) -> RgbaImage {
	let width = pixmap.width();
	let height = pixmap.height();
	let mut rgba_image = RgbaImage::new(width, height);
	for (new, old) in rgba_image.pixels_mut().zip(pixmap.pixels()) {
		let old = old.demultiply();
		*new = image::Rgba([old.red(), old.green(), old.blue(), old.alpha()])
	}
	rgba_image
}

fn svg_to_pixmap(data: Vec<u8>) -> resvg::tiny_skia::Pixmap {
	let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).unwrap();
	let tree = resvg::Tree::from_usvg(&tree);
	let mut pixmap =
		resvg::tiny_skia::Pixmap::new(EMOJI_SUPERSAMPLED_SIZE, EMOJI_SUPERSAMPLED_SIZE).unwrap();
	pixmap.fill(resvg::tiny_skia::Color::TRANSPARENT);
	let scale = EMOJI_SUPERSAMPLED_SIZE as f32 / tree.view_box.rect.width();
	tree.render(
		resvg::tiny_skia::Transform::from_scale(scale, scale),
		&mut pixmap.as_mut(),
	);
	pixmap
}

fn add_rotation_margin(image: RgbaImage) -> RgbaImage {
	let mut new_image = RgbaImage::new(
		EMOJI_SIZE_WITH_ROTATION_MARGIN,
		EMOJI_SIZE_WITH_ROTATION_MARGIN,
	);
	let corner = (EMOJI_SIZE_WITH_ROTATION_MARGIN - EMOJI_SUPERSAMPLED_SIZE) as i64 / 2;
	image::imageops::replace(&mut new_image, &image, corner, corner);
	new_image
}

fn rotate_randomly(image: &RgbaImage) -> RgbaImage {
	let angle = rand_distr::Normal::new(0.0, 0.125 * PI)
		.unwrap()
		.sample(&mut rand::thread_rng());
	imageproc::geometric_transformations::rotate_about_center(
		image,
		angle,
		imageproc::geometric_transformations::Interpolation::Bicubic,
		image::Rgba([0, 0, 0, 0]),
	)
}

fn place_randomly(canvas: &mut RgbaImage, images: &[(RgbaImage, i64)], count: usize) {
	let mut rng = rand::thread_rng();
	let canvas_width = canvas.width() as i64;
	let canvas_height = canvas.height() as i64;
	let width = EMOJI_SIZE as i64;
	let height = EMOJI_SIZE as i64;
	for _ in 0..count {
		for (image, image_count) in images {
			for _ in 0..*image_count {
				let x = rng.gen_range(0..canvas_width);
				let y = rng.gen_range(0..canvas_height);
				let image = rotate_randomly(image);
				let image = image::imageops::resize(
					&image,
					EMOJI_SIZE,
					EMOJI_SIZE,
					image::imageops::CatmullRom,
				);
				image::imageops::overlay(canvas, &image, x, y);
				if x + width > canvas_width {
					let x = x - canvas_width;
					image::imageops::overlay(canvas, &image, x, y);
				}
				if y + height > canvas_height {
					let y = y - canvas_height;
					image::imageops::overlay(canvas, &image, x, y);
				}
				if x + width > canvas_width && y + height > canvas_height {
					let x = x - canvas_width;
					let y = y - canvas_height;
					image::imageops::overlay(canvas, &image, x, y);
				}
			}
		}
	}
}

pub async fn test_image(
	emoji_map: &EmojiMap,
	context: Context,
	interaction: ApplicationCommandInteraction,
) {
	let input_emojis = interaction
		.data
		.options
		.get(0)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap()
		.trim();
	let emojis = match parse_emoji_input(emoji_map, input_emojis) {
		Ok(emojis) => emojis,
		Err(error) => {
			let _ = interaction_reply(context, interaction, error, true).await;
			return;
		}
	};

	let png = {
		let Some(images) = emojis.into_iter().map(|(emoji, count)| {
			let svg_data = read_emoji_svg(&emoji)?;
			let pixmap = svg_to_pixmap(svg_data);
			let image = pixmap_to_rgba_image(pixmap);
			Some((add_rotation_margin(image), count))
		}
		).collect::<Option<Vec<_>>>() else {
			let _ = interaction_reply(context, interaction, "Some file missing", true).await;
			return;
		};
		let mut canvas = RgbaImage::new(400, 200);
		place_randomly(&mut canvas, &images, 10);
		let mut bytes: Vec<u8> = Vec::new();
		canvas
			.write_to(
				&mut std::io::Cursor::new(&mut bytes),
				image::ImageOutputFormat::Png,
			)
			.unwrap();
		bytes
	};

	interaction
		.create_interaction_response(&context.http, |response| {
			response
				.kind(InteractionResponseType::ChannelMessageWithSource)
				.interaction_response_data(|message| {
					message.add_file((png.as_slice(), "image.png"))
				})
		})
		.await
		.unwrap();
}

pub fn register_test_image(
	command: &mut CreateApplicationCommand,
) -> &mut CreateApplicationCommand {
	command
		.name("testimage")
		.description("IDK")
		.default_member_permissions(Permissions::ADMINISTRATOR)
		.create_option(|option| {
			option
				.name("emoji")
				.description("The emoji to wibbly wobble.")
				.kind(CommandOptionType::String)
				.required(true)
		})
}

pub async fn command_generate(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: Context,
	interaction: ApplicationCommandInteraction,
) {
	let emojis = if let Some(input) = interaction
		.data
		.options
		.first()
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
	{
		let emojis = match parse_emoji_input(emoji_map, input) {
			Ok(emojis) => emojis,
			Err(message) => {
				let _ = interaction_reply(context, interaction, message, true).await;
				return;
			}
		};
		if !does_user_have_emojis(database, interaction.user.id, &emojis).await {
			let _ = interaction_reply(
				context,
				interaction,
				"You don't own all specified emojis.",
				true,
			)
			.await;
			return;
		}
		emojis
	} else {
		get_user_emojis(database, emoji_map, interaction.user.id).await
	};

	let Some(images) = emojis.into_iter().map(|(emoji, count)| {
		let svg_data = read_emoji_svg(&emoji)?;
		let pixmap = svg_to_pixmap(svg_data);
		let image = pixmap_to_rgba_image(pixmap);
		Some((add_rotation_margin(image), count))
	}
	).collect::<Option<Vec<_>>>() else {
		let _ = interaction_reply(context, interaction, "Some file missing.", true).await;
		return;
	};

	let mut canvas = RgbaImage::new(400, 200);
	place_randomly(&mut canvas, &images, 5);
	let mut bytes: Vec<u8> = Vec::new();
	canvas
		.write_to(
			&mut std::io::Cursor::new(&mut bytes),
			image::ImageOutputFormat::Png,
		)
		.unwrap();

	let _ = interaction
		.create_interaction_response(&context.http, |response| {
			response
				.kind(InteractionResponseType::ChannelMessageWithSource)
				.interaction_response_data(|message| {
					message.add_file((bytes.as_slice(), "image.png"))
				})
		})
		.await;
}

pub fn register_generate(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
	command
		.name("generate")
		.description("Generage an image using your emojis.")
		.create_option(|option| {
			option
				.name("emojis")
				.description("The emojis to use. If omitted, it uses all your emojis.")
				.kind(CommandOptionType::String)
				.required(false)
		})
}

pub async fn command_make_raster_image(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: Context,
	interaction: ApplicationCommandInteraction,
) {
	let input_emoji = interaction
		.data
		.options
		.get(0)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap()
		.trim();
	let Some(emoji) = emoji_map.get(input_emoji) else {
		interaction_reply(context, interaction, "No such emoji in my list", true).await.unwrap();
		return;
	};

	if !has_emoji(database, *emoji, interaction.user.id).await {
		interaction_reply(context, interaction, "You do not have that emoji.", true)
			.await
			.unwrap();
		return;
	}

	let png = {
		let size = 128;

		let Some(data) = read_emoji_svg(emoji) else {
			let _ = interaction_reply(context, interaction, "No such emoji in my files", true).await;
			return;
		};
		let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).unwrap();
		let tree = resvg::Tree::from_usvg(&tree);
		let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).unwrap();
		pixmap.fill(resvg::tiny_skia::Color::TRANSPARENT);
		let scale = size as f32 / tree.view_box.rect.width();
		tree.render(
			resvg::tiny_skia::Transform::from_scale(scale, scale),
			&mut pixmap.as_mut(),
		);
		pixmap.encode_png().unwrap()
	};

	let _ = interaction
		.create_interaction_response(&context.http, |response| {
			response
				.kind(InteractionResponseType::ChannelMessageWithSource)
				.interaction_response_data(|message| {
					message.add_file((png.as_slice(), "emoji.png"))
				})
		})
		.await;
}

pub fn register_make_raster_image(
	command: &mut CreateApplicationCommand,
) -> &mut CreateApplicationCommand {
	command
		.name("image")
		.description("Generates a raster image version of a specified emoji from your inventory.")
		.create_option(|option| {
			option
				.name("emoji")
				.description("The emoji to rasterize.")
				.kind(CommandOptionType::String)
				.required(true)
		})
}
