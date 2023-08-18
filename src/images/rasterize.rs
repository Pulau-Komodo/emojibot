//! Module for the command that rasterizes a single emoji.

use resvg::usvg::TreeParsing;
use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{
		application_command::ApplicationCommandInteraction, command::CommandOptionType,
		InteractionResponseType,
	},
	prelude::Context,
};
use sqlx::{Pool, Sqlite};

use crate::{emoji::EmojiMap, emojis_with_counts::EmojisWithCounts, util::interaction_reply};

use super::{queries::has_emojis, read_emoji_svg};

pub async fn execute(
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

	if !has_emojis(
		database,
		interaction.user.id,
		EmojisWithCounts::from_iter([(*emoji, 1)]),
	)
	.await
	{
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

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
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
