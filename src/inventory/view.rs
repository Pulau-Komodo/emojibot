use std::{borrow::Cow, fmt::Write};

use serenity::{
	all::{CommandDataOptionValue, CommandInteraction, CommandOptionType},
	builder::{CreateCommand, CreateCommandOption},
};

use crate::{
	context::Context, queries::get_user_emojis_grouped, user_settings::private::is_private,
	util::ReplyShortcuts,
};

pub async fn execute(context: Context<'_>, mut interaction: CommandInteraction) {
	let subcommand = interaction.data.options.pop().unwrap();
	let targets_own = subcommand.name == "own";
	let CommandDataOptionValue::SubCommand(options) = subcommand.value else {
		panic!("Received wrong option");
	};
	let target = if targets_own {
		interaction.user.id
	} else {
		options
			.first()
			.and_then(|option| option.value.as_user_id())
			.unwrap()
	};

	let is_public = if targets_own {
		options.get(0)
	} else {
		options.get(1)
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

pub fn register() -> CreateCommand {
	CreateCommand::new("inventory")
		.description("Check someone else's emoji inventory or your own.")
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::SubCommand,
				"own",
				"Check your own inventory.",
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::String,
					"show",
					"Whether to post your emojis publicly.",
				)
				.add_string_choice("show", "show")
				.required(false),
			),
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::SubCommand,
				"other",
				"Check someone else's inventory.",
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::User,
					"user",
					"Whose inventory to look at.",
				)
				.required(true),
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::String,
					"show",
					"Whether to post the emojis publicly.",
				)
				.add_string_choice("show", "show")
				.required(false),
			),
		)
}
