use std::fmt::Write;

use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{
		application_command::ApplicationCommandInteraction, command::CommandOptionType, UserId,
	},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{
	emoji::{Emoji, EmojiMap},
	util::interaction_reply,
};

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
		.map(|record| (UserId(record.user as u64), record.count))
		.collect()
}

pub async fn execute(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: Context,
	interaction: ApplicationCommandInteraction,
) {
	let options = &interaction.data.options.first().unwrap().options;
	let input = options
		.first()
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap_or("");

	let Some(&emoji) = emoji_map.get(input) else {
		let content = format!("Could not find \"{}\" as an emoji in my list.", input);
		let _ = interaction_reply(context, interaction, content, true).await;
		return;
	};

	let is_public = options.get(1).is_some();

	let users = find_emoji_users(database, emoji).await;

	if users.is_empty() {
		let _ = interaction_reply(
			context,
			interaction,
			format!("Nobody with a public inventory has {}.", emoji.as_str()),
			!is_public,
		)
		.await;
		return;
	}
	let mut output = if users.len() == 1 {
		format!(
			"The only user with a public inventory with {} is ",
			emoji.as_str()
		)
	} else {
		format!(
			"The users with public inventories with {} are ",
			emoji.as_str()
		)
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

	let _ = interaction_reply(context, interaction, output, !is_public).await;
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
	command
		.name("who")
		.description("Required field that shows up nowhere")
		.create_option(|option| {
			option
				.name("has")
				.description("Find all users with public inventories who own a specific emoji.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("emoji")
						.description("The emoji to look for.")
						.kind(CommandOptionType::String)
						.required(true)
				})
				.create_sub_option(|option| {
					option
						.name("show")
						.description("Whether to post the results publicly.")
						.add_string_choice("show", "show")
						.kind(CommandOptionType::String)
						.required(false)
				})
		})
}
