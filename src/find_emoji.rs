use std::fmt::Write;

use serenity::{
	all::{CommandDataOptionValue, CommandInteraction, CommandOptionType, UserId},
	builder::{CreateCommand, CreateCommandOption},
};
use sqlx::{query, Pool, Sqlite};

use crate::{context::Context, emoji::Emoji, util::ReplyShortcuts};

async fn find_emoji_users(executor: &Pool<Sqlite>, emoji: Emoji) -> Vec<(UserId, i64)> {
	let emoji = emoji.as_str();
	let result = query!(
		"
		SELECT emoji_inventory.user, COUNT(*) as count
		FROM emoji_inventory
			LEFT JOIN user_settings
			ON emoji_inventory.user = user_settings.user
		WHERE IFNULL(private, 0) = 0 AND emoji = ?
		GROUP BY emoji_inventory.user, emoji
		",
		emoji
	)
	.fetch_all(executor)
	.await
	.unwrap();

	result
		.into_iter()
		.map(|record| (UserId::new(record.user as u64), record.count))
		.collect()
}

pub async fn execute(context: Context<'_>, interaction: CommandInteraction) {
	let options = &interaction.data.options.first().unwrap().value;
	let CommandDataOptionValue::SubCommand(options) = options else {
		panic!()
	};
	let input = options
		.first()
		.and_then(|option| option.value.as_str())
		.unwrap_or("");

	let Some(emoji) = context.emoji_map.get(input) else {
		let content = format!("Could not find \"{}\" as an emoji in my list.", input);
		let _ = interaction.ephemeral_reply(context.http, content).await;
		return;
	};

	let is_public = options.get(1).is_some();

	let users = find_emoji_users(context.database, emoji).await;

	if users.is_empty() {
		let _ = interaction
			.reply(
				context.http,
				format!("Nobody with a public inventory has {}.", emoji),
				!is_public,
			)
			.await;
		return;
	}
	let mut output = if users.len() == 1 {
		format!("The only user with a public inventory with {} is ", emoji)
	} else {
		format!("The users with public inventories with {} are ", emoji)
	};

	let mut first = true;
	for (user, count) in users {
		if !first {
			output.push_str(", ");
		} else {
			first = false;
		}
		if !is_public && user == interaction.user.id {
			output.push_str("you");
		} else {
			write!(output, "<@{}>", user).unwrap();
		}
		if count > 1 {
			write!(output, " x{}", count).unwrap();
		}
	}
	output.push('.');

	let _ = interaction.reply(context.http, output, !is_public).await;
}

pub fn register() -> CreateCommand {
	CreateCommand::new("who")
		.description("Required field that shows up nowhere")
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::SubCommand,
				"has",
				"Find all users with public inventories who own a specific emoji.",
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::String,
					"emoji",
					"The emoji to look for.",
				)
				.required(true),
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::String,
					"show",
					"Whether to post the results publicly.",
				)
				.add_string_choice("show", "show")
				.required(false),
			),
		)
}
