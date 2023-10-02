//! Module for the commands that generate a randomized image with emojis.

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
};
use sqlx::{Pool, Sqlite};

use crate::{
	context::Context,
	emoji::EmojiMap,
	emojis_with_counts::EmojisWithCounts,
	inventory::queries::get_group_contents,
	util::{parse_emoji_input, ReplyShortcuts},
};

use super::read_emoji_svg;

const EMOJI_SIZE: u32 = 40;
/// (`EMOJI_SIZE`^2*2)^0.5, rounded up.
const EMOJI_SIZE_WITH_ROTATION_MARGIN: u32 = 57;
const EMOJI_SUPERSAMPLED_SIZE: u32 = EMOJI_SIZE * 2;
const EMOJI_SUPERSAMPLED_SIZE_WITH_ROTATION_MARGIN: u32 = 114;

const CANVAS_WIDTH: u32 = 500;
const CANVAS_HEIGHT: u32 = 250;
const EMOJI_REPETITION: usize = 5;

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

fn svg_to_pixmap(data: &[u8]) -> resvg::tiny_skia::Pixmap {
	let tree = resvg::usvg::Tree::from_data(data, &resvg::usvg::Options::default()).unwrap();
	let tree = resvg::Tree::from_usvg(&tree);
	let mut pixmap = resvg::tiny_skia::Pixmap::new(
		EMOJI_SUPERSAMPLED_SIZE_WITH_ROTATION_MARGIN,
		EMOJI_SUPERSAMPLED_SIZE_WITH_ROTATION_MARGIN,
	)
	.unwrap();
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
		EMOJI_SUPERSAMPLED_SIZE_WITH_ROTATION_MARGIN,
		EMOJI_SUPERSAMPLED_SIZE_WITH_ROTATION_MARGIN,
	);
	let corner = (EMOJI_SUPERSAMPLED_SIZE_WITH_ROTATION_MARGIN - EMOJI_SUPERSAMPLED_SIZE) as i64 / 2;
	image::imageops::replace(&mut new_image, &image, corner, corner);
	new_image
}

fn random_angle(rng: &mut rand::rngs::ThreadRng) -> f32 {
	rand_distr::Normal::new(0.0, 0.125 * PI)
		.unwrap()
		.sample(rng)
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

fn place_randomly(canvas: &mut RgbaImage, images: &[RgbaImage], count: usize) {
	let mut rng = rand::thread_rng();
	let canvas_width = canvas.width() as i64;
	let canvas_height = canvas.height() as i64;
	let width = EMOJI_SIZE_WITH_ROTATION_MARGIN as i64;
	let height = EMOJI_SIZE_WITH_ROTATION_MARGIN as i64;
	for _ in 0..count {
		for image in images {
			let x = rng.gen_range(0..canvas_width);
			let y = rng.gen_range(0..canvas_height);
			let image = rotate_randomly(image);
			let image = image::imageops::resize(
				&image,
				EMOJI_SIZE_WITH_ROTATION_MARGIN,
				EMOJI_SIZE_WITH_ROTATION_MARGIN,
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

fn place_emoji_randomly(
	canvas: &mut resvg::tiny_skia::PixmapMut,
	tree: &resvg::Tree,
	rng: &mut rand::rngs::ThreadRng,
) {
	let canvas_width = canvas.width() as f32;
	let canvas_height = canvas.height() as f32;
	let width = EMOJI_SIZE_WITH_ROTATION_MARGIN as f32;
	let height = EMOJI_SIZE_WITH_ROTATION_MARGIN as f32;
	let x = rng.gen_range(0.0..canvas_width)
		+ (EMOJI_SIZE_WITH_ROTATION_MARGIN - EMOJI_SIZE) as f32 / 2.0; // Add half the rotation margin so rotation can't make it go over the left or top edges.
	let y = rng.gen_range(0.0..canvas_height)
		+ (EMOJI_SIZE_WITH_ROTATION_MARGIN - EMOJI_SIZE) as f32 / 2.0;

	let scale = EMOJI_SIZE as f32 / tree.view_box.rect.width();
	let angle = random_angle(rng).to_degrees();
	let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).pre_rotate_at(
		angle,
		tree.view_box.rect.width() / 2.0,
		tree.view_box.rect.height() / 2.0,
	);

	tree.render(transform.post_translate(x, y), canvas);

	if x + width > canvas_width {
		let x = x - canvas_width;
		tree.render(transform.post_translate(x, y), canvas);
	}
	if y + height > canvas_height {
		let y = y - canvas_height;
		tree.render(transform.post_translate(x, y), canvas);
	}
	if x + width > canvas_width && y + height > canvas_height {
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

pub async fn execute_test(context: Context<'_>, interaction: ApplicationCommandInteraction) {
	let input = interaction
		.data
		.options
		.first()
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
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
	let emojis = emojis.flatten();

	let time = std::time::Instant::now();

	let png = {
		let Some(images) = emojis
			.into_iter()
			.map(|emoji| {
				// let svg_data = read_emoji_svg(&emoji)?;
				// let pixmap = svg_to_pixmap(&svg_data);
				// let image = pixmap_to_rgba_image(pixmap);
				let path = std::path::PathBuf::from(format!("./assets/png/{}", emoji.file_name_png()));
				image::open(path).ok().map(|image| image.into_rgba8())
				// let image = add_rotation_margin(image);
				// let _ = image.save(std::path::PathBuf::from(format!("./assets/png/{}", emoji.file_name_png()))).map_err(|err| println!("{:?}", err));
				// Some(image)
			})
			.collect::<Option<Vec<_>>>()
		else {
			let _ = interaction
				.ephemeral_reply(context.http, "Some file missing")
				.await;
			return;
		};
		let mut canvas = RgbaImage::new(CANVAS_WIDTH, CANVAS_HEIGHT);
		place_randomly(&mut canvas, &images, EMOJI_REPETITION);
		println!(".png: {:?}", time.elapsed());
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

pub fn register_test(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
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

pub async fn execute(context: Context<'_>, interaction: ApplicationCommandInteraction) {
	let input = interaction
		.data
		.options
		.first()
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
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

	let time = std::time::Instant::now();

	let mut canvas = resvg::tiny_skia::Pixmap::new(CANVAS_WIDTH, CANVAS_HEIGHT).unwrap();
	{
		let Some(image_trees) = emojis
			.flatten()
			.into_iter()
			.map(|emoji| {
				let svg = read_emoji_svg(&emoji)?;
				let tree =
					resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default()).unwrap();
				Some(resvg::Tree::from_usvg(&tree))
			})
			.collect::<Option<Vec<_>>>()
		else {
			let _ = interaction
				.ephemeral_reply(context.http, "Some file missing.")
				.await;
			return;
		};

		let mut rng = rand::thread_rng();
		let canvas_mut = &mut canvas.as_mut();
		for _ in 0..EMOJI_REPETITION {
			for tree in &image_trees {
				place_emoji_randomly(canvas_mut, tree, &mut rng);
			}
		}
	}

	let elapsed = time.elapsed();

	let image = pixmap_to_rgba_image(canvas);

	println!(".svg: {:?}, {:?}", elapsed, time.elapsed());

	let mut bytes: Vec<u8> = Vec::new();
	image
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

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
	command
		.name("generate")
		.description("Generage an image using your emojis.")
		.create_option(|option| {
			option
				.name("emojis")
				.description("The emojis to use. You can use emojis and emoji groups together, comma-separated.")
				.kind(CommandOptionType::String)
				.required(true)
		})
}
