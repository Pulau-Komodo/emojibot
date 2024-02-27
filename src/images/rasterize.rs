//! Module for the command that rasterizes a single emoji.

use serenity::{
	all::{CommandInteraction, CommandOptionType},
	builder::{CreateCommand, CreateCommandOption},
};

use crate::{context::Context, emojis_with_counts::EmojisWithCounts, util::ReplyShortcuts};

pub async fn execute(context: Context<'_>, interaction: CommandInteraction) {
	let input_emoji = interaction
		.data
		.options
		.get(0)
		.and_then(|option| option.value.as_str())
		.unwrap()
		.trim();
	let Some(emoji) = context.emoji_map.get_with_image(input_emoji) else {
		let _ = interaction
			.ephemeral_reply(context.http, "No such emoji in my list.")
			.await;
		return;
	};

	if !EmojisWithCounts::from_iter([(emoji.emoji(), 1)])
		.are_owned_by_user(context.database, interaction.user.id)
		.await
	{
		let _ = interaction
			.ephemeral_reply(context.http, "You do not have that emoji.")
			.await;
		return;
	}

	let png = {
		let size = 128;
		let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).unwrap();
		pixmap.fill(resvg::tiny_skia::Color::TRANSPARENT);
		let scale = size as f32 / emoji.image().view_box().rect.width();
		emoji.render(
			resvg::tiny_skia::Transform::from_scale(scale, scale),
			&mut pixmap.as_mut(),
		);
		pixmap.encode_png().unwrap()
	};

	let _ = interaction
		.reply_image(context.http, png.as_slice(), "emoji.png")
		.await;
}

pub fn register() -> CreateCommand {
	CreateCommand::new("image")
		.description("Generates a raster image version of a specified emoji from your inventory.")
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"emoji",
				"The emoji to rasterize.",
			)
			.required(true),
		)
}
