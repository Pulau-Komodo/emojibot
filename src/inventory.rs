use std::{borrow::Cow, fmt::Write};

use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{
		application_command::ApplicationCommandInteraction, command::CommandOptionType, UserId,
	},
	prelude::Context,
};
use sqlx::{Pool, Sqlite};

use crate::{
	emoji::EmojiMap,
	queries::get_user_emojis,
	special_characters::ZWNJ,
	user_settings::private::is_private,
	util::{get_name, interaction_reply},
};

pub async fn command(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: Context,
	mut interaction: ApplicationCommandInteraction,
) {
	let subcommand = interaction.data.options.pop().unwrap();
	let targets_own = subcommand.name == "own";
	let target = if targets_own {
		interaction.user.id
	} else {
		let option = subcommand.options.get(0).unwrap();
		let id = option
			.value
			.as_ref()
			.and_then(|value| value.as_str())
			.unwrap();
		UserId(id.parse().unwrap())
	};

	let is_public = if targets_own {
		subcommand.options.get(0)
	} else {
		subcommand.options.get(1)
	}
	.is_some();

	let name = if !targets_own || is_public {
		Some(get_name(&context, interaction.guild_id.unwrap(), target).await)
	} else {
		None
	};

	if !targets_own && is_private(database, target).await {
		let _ = interaction_reply(
			context,
			interaction,
			format!("{}'s inventory is set to private.", name.unwrap()),
			!is_public,
		)
		.await;
		return;
	}

	let emojis = get_user_emojis(database, emoji_map, target).await;
	if emojis.is_empty() {
		let message = name
			.map(|name| Cow::from(format!("{name} has no emojis. 🤔")))
			.unwrap_or_else(|| Cow::from("You have no emojis. 🤔"));
		interaction_reply(context, interaction, message, !is_public)
			.await
			.unwrap();
		return;
	}
	let mut output = match (emojis.len(), name) {
		(1, Some(name)) => format!("{name} only has "),
		(1, None) => String::from("You only have "),
		(_, Some(name)) => format!("{name} has the following emojis: "),
		(_, None) => String::from("You have the following emojis: "),
	};

	for (emoji, count) in emojis {
		output.push_str(emoji.as_str());
		if count > 1 {
			write!(output, "x{count}").unwrap();
		} else {
			output.push_str(ZWNJ); // To avoid some emojis combining inappropriately.
		}
	}
	output.push('.');
	interaction_reply(context, interaction, output, !is_public)
		.await
		.unwrap();
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
	command
		.name("inventory")
		.description("Check someone else's emoji inventory or your own.")
		.create_option(|option| {
			option
				.name("own")
				.description("Check your own inventory.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("show")
						.description("Whether to post your emojis publicly.")
						.add_string_choice("show", "show")
						.kind(CommandOptionType::String)
						.required(false)
				})
		})
		.create_option(|option| {
			option
				.name("other")
				.description("Check someone else's inventory.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("user")
						.description("Whose inventory to look at.")
						.kind(CommandOptionType::User)
						.required(true)
				})
				.create_sub_option(|option| {
					option
						.name("show")
						.description("Whether to post the emojis publicly.")
						.add_string_choice("show", "show")
						.kind(CommandOptionType::String)
						.required(false)
				})
		})
}
