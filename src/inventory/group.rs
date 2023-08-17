use std::fmt::Write;

use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{
		application_command::{ApplicationCommandInteraction, CommandDataOption},
		command::CommandOptionType,
		UserId,
	},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite, Transaction};

use crate::{
	emoji::EmojiMap,
	emojis_with_counts::EmojisWithCounts,
	util::{ephemeral_reply, get_and_parse_emoji_option},
};

pub async fn remove_empty_groups(executor: &mut Transaction<'_, Sqlite>, user: UserId) {
	let user_id = user.0 as i64;
	let deleted_any = query!(
		"
		DELETE FROM emoji_inventory_groups
		WHERE user = ? AND (
			SELECT COUNT(*)
			FROM emoji_inventory
			WHERE emoji_inventory.user = emoji_inventory_groups.user AND emoji_inventory.group_id = emoji_inventory_groups.id
		) = 0
		", user_id
	).execute(&mut **executor).await.unwrap().rows_affected() > 0;
	if deleted_any {
		close_ordering_gaps(executor, user).await;
	}
}

async fn close_ordering_gaps(executor: &mut Transaction<'_, Sqlite>, user: UserId) {
	let user_id = user.0 as i64;
	let groups = query!(
		"
		SELECT name
		FROM emoji_inventory_groups
		WHERE user = ?
		ORDER BY sort_order ASC
		",
		user_id
	)
	.fetch_all(&mut **executor)
	.await
	.unwrap();

	for (index, group) in groups.into_iter().enumerate() {
		let group_name = group.name;
		let sort_order = index as i64;
		query!(
			"
			UPDATE emoji_inventory_groups
			SET sort_order = ?
			WHERE user = ? AND name = ?
			",
			sort_order,
			user_id,
			group_name
		)
		.execute(&mut **executor)
		.await
		.unwrap();
	}
}

async fn add_to_group(
	executor: &Pool<Sqlite>,
	user: UserId,
	group_name: &str,
	emojis: &EmojisWithCounts,
) -> (String, EmojisWithCounts) {
	let user_id = user.0 as i64;
	let mut transaction = executor.begin().await.unwrap();
	let group_count = query!(
		"
		SELECT COUNT(*) AS group_count
		FROM emoji_inventory_groups
		WHERE user = ?
		",
		user_id
	)
	.fetch_one(&mut *transaction)
	.await
	.unwrap()
	.group_count;
	let group = query!(
		"
		INSERT INTO emoji_inventory_groups (user, name, sort_order)
		VALUES (?, ?, ?)
		ON CONFLICT (user, name COLLATE NOCASE) DO NOTHING;
		SELECT id, name
		FROM emoji_inventory_groups
		WHERE user = ? AND name = ?;
		",
		user_id,
		group_name,
		group_count,
		user_id,
		group_name,
	)
	.fetch_one(&mut *transaction)
	.await
	.unwrap();

	let group_id = group.id;
	let mut added_emojis = Vec::with_capacity(emojis.unique_emoji_count());
	for (emoji, count) in emojis {
		let emoji_str = emoji.as_str();
		let rows_affected = query!(
			"
			UPDATE emoji_inventory
			SET group_id = ?
			WHERE user = ? AND emoji = ? AND rowid IN (
				SELECT emoji_inventory.rowid
				FROM emoji_inventory
				LEFT JOIN emoji_inventory_groups
				ON emoji_inventory.group_id = emoji_inventory_groups.id
				WHERE emoji_inventory.user = ? AND emoji_inventory.emoji = ?
				ORDER BY IFNULL(sort_order, 9223372036854775807) DESC
				LIMIT ?
			)
			",
			group_id,
			user_id,
			emoji_str,
			user_id,
			emoji_str,
			*count
		)
		.execute(&mut *transaction)
		.await
		.unwrap()
		.rows_affected();
		if rows_affected > 0 {
			added_emojis.push((*emoji, rows_affected as u32));
		}
	}

	remove_empty_groups(&mut transaction, user).await;

	transaction.commit().await.unwrap();

	(group.name, EmojisWithCounts::new(added_emojis))
}

async fn remove_from_group(
	executor: &Pool<Sqlite>,
	user: UserId,
	emojis: &EmojisWithCounts,
	group: &str,
) -> EmojisWithCounts {
	let user_id = user.0 as i64;
	let mut degrouped_emojis = Vec::new();

	let mut transaction = executor.begin().await.unwrap();
	for (emoji, count) in emojis {
		let emoji_str = emoji.as_str();
		let rows_affected = query!(
			"
			UPDATE emoji_inventory
			SET group_id = NULL
			WHERE user = ? AND emoji = ? AND emoji_inventory.group_id IS NOT NULL AND rowid IN (
				SELECT emoji_inventory.rowid
				FROM emoji_inventory
				LEFT JOIN emoji_inventory_groups
				ON emoji_inventory.group_id = emoji_inventory_groups.id
				WHERE emoji_inventory.user = ? AND emoji_inventory.emoji = ? AND emoji_inventory_groups.name = ?
				ORDER BY sort_order DESC
				LIMIT ?
			)
			",
			user_id,
			emoji_str,
			user_id,
			emoji_str,
			group,
			count
		)
		.execute(&mut *transaction)
		.await
		.unwrap()
		.rows_affected();
		if rows_affected > 0 {
			degrouped_emojis.push((*emoji, rows_affected as u32));
		}
	}

	remove_empty_groups(&mut transaction, user).await;

	transaction.commit().await.unwrap();

	EmojisWithCounts::new(degrouped_emojis)
}

pub async fn execute(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: Context,
	mut interaction: ApplicationCommandInteraction,
) {
	let subcommand = interaction.data.options.pop().unwrap();
	match subcommand.name.as_str() {
		"add" => {
			add(
				database,
				emoji_map,
				context,
				interaction,
				subcommand.options,
			)
			.await
		}
		"remove" => {
			remove(
				database,
				emoji_map,
				context,
				interaction,
				subcommand.options,
			)
			.await
		}
		"rename" | "list" | "view" | "reposition" => {
			let _ = ephemeral_reply(context, interaction, "Not yet implemented.").await;
		}
		_ => panic!("Received invalid subcommand name."),
	}
}

async fn add(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: Context,
	interaction: ApplicationCommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let group_name = options
		.get(0)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap();

	let emojis = match get_and_parse_emoji_option(emoji_map, &options, 1) {
		Ok(emojis) => emojis,
		Err(error) => {
			let _ = ephemeral_reply(context, interaction, error).await;
			return;
		}
	};

	let emojis = EmojisWithCounts::from_flat(&emojis);
	let emoji_count = emojis.emoji_count();

	let (group_name, added_emojis) =
		add_to_group(database, interaction.user.id, group_name, &emojis).await;

	if added_emojis.is_empty() {
		let message = match emoji_count {
			1 => "You do not have that emoji.",
			2 => "You do not have either of those emojis.",
			_ => "You did not have any of those emojis.",
		};
		let _ = ephemeral_reply(context, interaction, message).await;
		return;
	}

	let dropped_emojis = emoji_count - added_emojis.emoji_count();

	let mut message = format!("Added {} to {}.", added_emojis, group_name);

	match dropped_emojis {
		0 => (),
		1 => message.push_str(" You did not have the other one."),
		n => write!(message, " You did not have the other {}.", n).unwrap(),
	}

	let _ = ephemeral_reply(context, interaction, message).await;
}

async fn remove(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: Context,
	interaction: ApplicationCommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let group = options
		.get(0)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap();
	let emojis = match get_and_parse_emoji_option(emoji_map, &options, 1) {
		Ok(emojis) => emojis,
		Err(error) => {
			let _ = ephemeral_reply(context, interaction, error).await;
			return;
		}
	};

	let emojis = EmojisWithCounts::from_flat(&emojis);
	let emoji_count = emojis.emoji_count();

	let degrouped_emojis = remove_from_group(database, interaction.user.id, &emojis, group).await;

	if degrouped_emojis.is_empty() {
		let message = match emoji_count {
			1 => "That emoji is not in that group.",
			2 => "Neither of those emojis are in that group.",
			_ => "None of those emojis are in that group.",
		};
		let _ = ephemeral_reply(context, interaction, message).await;
		return;
	}

	let skipped_emojis = emoji_count - degrouped_emojis.emoji_count();

	let mut message = format!("{}", degrouped_emojis);
	if degrouped_emojis.emoji_count() == 1 {
		message.push_str(" is");
	} else {
		message.push_str(" are");
	}
	message.push_str(" now ungrouped.");

	match skipped_emojis {
		0 => (),
		1 => message.push_str(" The other one was not in that group."),
		n => write!(message, " The other {} were not in that group.", n).unwrap(),
	}

	let _ = ephemeral_reply(context, interaction, message).await;
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
	command
		.name("group")
		.description("Interact with emoji groups.")
		.create_option(|option| {
			option
				.name("add")
				.description("Add emojis to a group. Makes the group if it doesn't already exist.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("group")
						.description("The group to add the emojis to.")
						.kind(CommandOptionType::String)
						.max_length(50)
						.required(true)
				})
				.create_sub_option(|option| {
					option
						.name("emojis")
						.description("The emojis to add to the group.")
						.kind(CommandOptionType::String)
						.required(true)
				})
		})
		.create_option(|option| {
			option
				.name("remove")
				.description("Removes emojis from a group.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("group")
						.description("The group to remove the emojis from.")
						.kind(CommandOptionType::String)
						.max_length(50)
						.required(true)
				})
				.create_sub_option(|option| {
					option
						.name("emojis")
						.description("The emojis to remove from the group.")
						.kind(CommandOptionType::String)
						.required(true)
				})
		})
		.create_option(|option| {
			option
				.name("rename")
				.description("Renames an emoji group.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("group")
						.description("The group to rename.")
						.kind(CommandOptionType::String)
						.max_length(50)
						.required(true)
				})
				.create_sub_option(|option| {
					option
						.name("new_name")
						.description("The new name for the group.")
						.kind(CommandOptionType::String)
						.max_length(50)
						.required(true)
				})
		})
		.create_option(|option| {
			option
				.name("list")
				.description("Lists all your emoji groups.")
				.kind(CommandOptionType::SubCommand)
		})
		.create_option(|option| {
			option
				.name("view")
				.description("Views the contents of one of your emoji groups.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("group")
						.description("The group to view the contents of.")
						.kind(CommandOptionType::String)
						.max_length(50)
						.required(true)
				})
		})
		.create_option(|option| {
			option
				.name("reposition")
				.description("Repositions the group in the group list. This is mostly relevant when viewing inventory.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("group")
						.description("The group to reposition.")
						.kind(CommandOptionType::String)
						.max_length(50)
						.required(true)
				})
				.create_sub_option(|option| {
					option
						.name("position")
						.description("The position to move the group to, where 0 is the first.")
						.kind(CommandOptionType::Integer)
						.required(true)
				})
		})
}
