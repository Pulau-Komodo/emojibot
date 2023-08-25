use std::fmt::Write;

use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{
		application_command::{ApplicationCommandInteraction, CommandDataOption},
		command::CommandOptionType,
	},
};

use crate::{
	context::Context,
	emojis_with_counts::EmojisWithCounts,
	util::{get_and_parse_emoji_option, ReplyShortcuts},
};

use super::queries::{
	add_to_group, group_name_and_contents, list_groups, remove_from_group, rename_group,
	reposition_group, RepositionOutcome,
};

pub async fn execute(context: Context<'_>, mut interaction: ApplicationCommandInteraction) {
	let subcommand = interaction.data.options.pop().unwrap();
	match subcommand.name.as_str() {
		"add" => {
			add(context, interaction, subcommand.options).await;
		}
		"remove" => {
			remove(context, interaction, subcommand.options).await;
		}
		"rename" => {
			rename(context, interaction, subcommand.options).await;
		}
		"list" => {
			list(context, interaction).await;
		}
		"view" => {
			view(context, interaction, subcommand.options).await;
		}
		"reposition" => {
			// let _ = ephemeral_reply(context, interaction, "Not yet implemented.").await;
			// return;
			reposition(context, interaction, subcommand.options).await;
		}
		_ => panic!("Received invalid subcommand name."),
	}
}

async fn add(
	context: Context<'_>,
	interaction: ApplicationCommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let group_name = options
		.get(0)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap();

	let emojis = match get_and_parse_emoji_option(context.emoji_map, &options, 1) {
		Ok(emojis) => emojis,
		Err(error) => {
			let _ = interaction.ephemeral_reply(context.http, error).await;
			return;
		}
	};

	let emojis = EmojisWithCounts::from_flat(&emojis);
	let emoji_count = emojis.emoji_count();

	let (group_name, added_emojis) =
		add_to_group(context.database, interaction.user.id, group_name, &emojis).await;

	if added_emojis.is_empty() {
		let message = match emoji_count {
			1 => "You do not have that emoji.",
			2 => "You do not have either of those emojis.",
			_ => "You did not have any of those emojis.",
		};
		let _ = interaction.ephemeral_reply(context.http, message).await;
		return;
	}

	let dropped_emojis = emoji_count - added_emojis.emoji_count();

	let mut message = format!("Added {} to {}.", added_emojis, group_name);

	match dropped_emojis {
		0 => (),
		1 => message.push_str(" You did not have the other one."),
		n => write!(message, " You did not have the other {}.", n).unwrap(),
	}

	let _ = interaction.ephemeral_reply(context.http, message).await;
}

async fn remove(
	context: Context<'_>,
	interaction: ApplicationCommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let subcommand = options.get(0).unwrap();
	let group = (subcommand.name.as_str() == "from").then(|| {
		subcommand
			.options
			.get(0)
			.and_then(|option| option.value.as_ref())
			.and_then(|value| value.as_str())
			.unwrap()
	});
	let emojis_index = if group.is_some() { 1 } else { 0 };
	let emojis =
		match get_and_parse_emoji_option(context.emoji_map, &subcommand.options, emojis_index) {
			Ok(emojis) => emojis,
			Err(error) => {
				let _ = interaction.ephemeral_reply(context.http, error).await;
				return;
			}
		};

	let emoji_count = emojis.len() as u32;
	let emojis = EmojisWithCounts::from_flat(&emojis);

	let degrouped_emojis =
		remove_from_group(context.database, interaction.user.id, &emojis, group).await;

	if degrouped_emojis.is_empty() {
		let message = match (emoji_count, group.is_some()) {
			(1, true) => "That emoji is not in that group.",
			(2, true) => "Neither of those emojis are in that group.",
			(_, true) => "None of those emojis are in that group.",
			(1, false) => "You do not have that emoji.",
			(2, false) => "You do not have either of those emojis.",
			(_, false) => "You don't have any of those emojis.",
		};
		let _ = interaction.ephemeral_reply(context.http, message).await;
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

	match (skipped_emojis, group.is_some()) {
		(0, _) => (),
		(1, true) => message.push_str(" The other one was not in that group."),
		(n, true) => write!(message, " The other {} were not in that group.", n).unwrap(),
		(1, false) => message.push_str(" You did not have the other one."),
		(n, false) => write!(message, " You did not have the other {}.", n).unwrap(),
	}

	let _ = interaction.ephemeral_reply(context.http, message).await;
}

async fn rename(
	context: Context<'_>,
	interaction: ApplicationCommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let group = options
		.get(0)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap();
	let new_name = options
		.get(1)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap();

	let Ok(old_name) = rename_group(context.database, interaction.user.id, group, new_name).await
	else {
		let _ = interaction
			.ephemeral_reply(
				context.http,
				format!("You have no group called \"{group}\"."),
			)
			.await;
		return;
	};

	let message = format!("Renamed group {} to {}.", old_name, new_name);
	let _ = interaction.ephemeral_reply(context.http, message).await;
}

async fn list(context: Context<'_>, interaction: ApplicationCommandInteraction) {
	let (groups, ungrouped) = list_groups(context.database, interaction.user.id).await;

	let s = if ungrouped == 1 { "" } else { "s" };
	let message = match groups.len() {
		0 => format!("You have no groups and {ungrouped} ungrouped emoji{s}."),
		1 => {
			let (name, count) = groups.first().unwrap();
			format!(
				"Your only group is {} ({}) and you have {ungrouped} ungrouped emoji{s}.",
				name, count
			)
		}
		group_count => {
			let mut message = String::from("Your groups are ");
			for (index, (name, count)) in groups.into_iter().enumerate() {
				if index + 1 == group_count {
					message.push_str(" and ");
				} else if index != 0 {
					message.push_str(", ");
				}
				message
					.write_fmt(format_args!("{} ({})", name, count))
					.unwrap();
			}
			message
				.write_fmt(format_args!(
					", and you have {ungrouped} ungrouped emoji{s}."
				))
				.unwrap();
			message
		}
	};

	let _ = interaction.ephemeral_reply(context.http, message).await;
}

async fn view(
	context: Context<'_>,
	interaction: ApplicationCommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let group = options
		.get(0)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap();

	let Some((name, emojis)) = group_name_and_contents(
		context.database,
		context.emoji_map,
		interaction.user.id,
		group,
	)
	.await
	else {
		_ = interaction
			.ephemeral_reply(
				context.http,
				format!("You have no group called \"{group}\"."),
			)
			.await;
		return;
	};

	let message = format!("Contents of group {}: {}", name, emojis);
	_ = interaction.ephemeral_reply(context.http, message).await;
}

async fn reposition(
	context: Context<'_>,
	interaction: ApplicationCommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let group = options
		.get(0)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap();
	let Some::<u32>(new_position) = options
		.get(1)
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_u64())
		.and_then(|num| num.try_into().ok())
	else {
		eprintln!("Somehow received a number that can't be represented as u32.");
		let _ = interaction
			.ephemeral_reply(context.http, "Error: invalid number.")
			.await;
		return;
	};

	let Ok((name, outcome, old_position, group_count)) =
		reposition_group(context.database, interaction.user.id, group, new_position).await
	else {
		let _ = interaction
			.ephemeral_reply(
				context.http,
				format!("You have no group called \"{group}\"."),
			)
			.await;
		return;
	};

	let message = if group_count == 1 {
		format!("{name} is your only group. There is nowhere to move it.")
	} else {
		match outcome {
			RepositionOutcome::DidNotMove => {
				if new_position > group_count {
					format!("That move just puts {name} at the end, where it already was.")
				} else if old_position == group_count - 1 {
					format!("{name} was already at the end.")
				} else if new_position == 0 {
					format!("{name} was already at the start.")
				} else {
					format!("{name} was already in that position.")
				}
			}
			RepositionOutcome::MovedToFront => {
				format!("Moved {name} to the start.")
			}
			RepositionOutcome::MovedToBack => {
				format!("Moved {name} to the end.")
			}
			RepositionOutcome::MovedBetween(neighbours) => {
				if new_position > old_position {
					format!(
						"Moved {name} down between {} and {}.",
						neighbours[0], neighbours[1]
					)
				} else {
					format!(
						"Moved {name} up between {} and {}.",
						neighbours[0], neighbours[1]
					)
				}
			}
		}
	};

	let _ = interaction.ephemeral_reply(context.http, message).await;
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
				.kind(CommandOptionType::SubCommandGroup)
				.create_sub_option(|option| {
					option
						.name("from")
						.description("Removes emojis from a specific group.")
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
				.create_sub_option(|option| {
					option
						.name("emojis")
						.description("Removes emojis from whatever group.")
						.kind(CommandOptionType::SubCommand)
						.create_sub_option(|option| {
							option
								.name("emojis")
								.description("The emojis to remove from whatever group they're in.")
								.kind(CommandOptionType::String)
								.required(true)
						})
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
						.min_int_value(0)
						.required(true)
				})
		})
}
