use std::{borrow::Cow, fmt::Write};

use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{
		application_command::ApplicationCommandInteraction, command::CommandOptionType, UserId,
	},
};

use crate::{
	context::Context, queries::get_user_emojis_grouped, user_settings::private::is_private,
	util::ReplyShortcuts,
};

pub async fn execute(context: Context<'_>, mut interaction: ApplicationCommandInteraction) {
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
		Some(
			context
				.get_user_name(interaction.guild_id.unwrap(), target)
				.await,
		)
	} else {
		None
	};

	if !targets_own && is_private(context.database, target).await {
		let _ = interaction
			.reply(
				context.http,
				format!("{}'s inventory is set to private.", name.unwrap()),
				!is_public,
			)
			.await;
		return;
	}

	let (groups, ungrouped) =
		get_user_emojis_grouped(context.database, context.emoji_map, target).await;
	let emoji_count = groups
		.iter()
		.chain(&ungrouped)
		.fold(0, |sum, emoji_set| sum + emoji_set.emoji_count());
	if emoji_count == 0 {
		let message = name
			.map(|name| Cow::from(format!("{name} has no emojis. 🤔")))
			.unwrap_or_else(|| Cow::from("You have no emojis. 🤔"));
		interaction
			.reply(context.http, message, !is_public)
			.await
			.unwrap();
		return;
	}
	let mut output = match (emoji_count, name) {
		(1, Some(name)) => format!("{name} only has "),
		(1, None) => String::from("You only have "),
		(n, Some(name)) => format!("{name} has the following {n} emojis: "),
		(n, None) => format!("You have the following {n} emojis: "),
	};

	for emojis in groups.into_iter() {
		output.write_fmt(format_args!("[{}]", emojis)).unwrap();
	}
	for emojis in ungrouped.into_iter() {
		output.write_fmt(format_args!("{}", emojis)).unwrap();
	}

	output.push('.');
	interaction
		.reply(context.http, output, !is_public)
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
