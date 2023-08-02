use resvg::usvg::TreeParsing;
use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{
		application_command::ApplicationCommandInteraction, command::CommandOptionType,
		InteractionResponseType,
	},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{emojis::EmojiMap, util::interaction_reply};

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
		println!("\"{input_emoji}\" was not found in the emoji map.");
		interaction_reply(context, interaction, "No such emoji in my list", true).await.unwrap();
		return;
	};

	let user_id = interaction.user.id.0 as i64;
	let emoji_str = emoji.as_str();
	let has_emoji = query!(
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
	.fetch_optional(database)
	.await
	.unwrap()
	.map(|record| record.count > 0)
	.unwrap_or(false);

	if !has_emoji {
		interaction_reply(context, interaction, "You do not have that emoji.", true)
			.await
			.unwrap();
		return;
	}

	let png = {
		let size = 128;

		let Ok(data) = std::fs::read(String::from("./assets/svg/") + &emoji.file_name()) else {
			eprintln!("\"{input_emoji}\" was not found in the emoji .svg files.");
			interaction_reply(context, interaction, "No such emoji in my files", true).await.unwrap();
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
	interaction
		.create_interaction_response(&context.http, |response| {
			response
				.kind(InteractionResponseType::ChannelMessageWithSource)
				.interaction_response_data(|message| {
					message.add_file((png.as_slice(), "emoji.png"))
				})
		})
		.await
		.unwrap();
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
