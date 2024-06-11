use std::fmt::Write;

use serenity::{
	all::{CommandDataOption, CommandDataOptionValue, CommandInteraction, CommandOptionType},
	builder::{CreateCommand, CreateCommandOption},
};

use crate::{
	context::Context,
	emojis_with_counts::EmojisWithCounts,
	util::{get_and_parse_emoji_option, ReplyShortcuts},
};

use super::queries::{
	add_to_group, get_ungrouped_emojis, group_name_and_contents, list_groups, remove_from_group,
	rename_group, reposition_group, RenameGroupError, RepositionOutcome,
};

pub async fn execute(context: Context<'_>, mut interaction: CommandInteraction) {
	let subcommand = interaction.data.options.pop().unwrap();
	let options = match subcommand.value {
		CommandDataOptionValue::SubCommand(options) => options,
		CommandDataOptionValue::SubCommandGroup(options) => options,
		_ => panic!("Received wrong argument"),
	};
	match subcommand.name.as_str() {
		"add" => {
			add(context, interaction, options).await;
		}
		"remove" => {
			remove(context, interaction, options).await;
		}
		"rename" => {
			rename(context, interaction, options).await;
		}
		"list" => {
			list(context, interaction).await;
		}
		"view" => {
			view(context, interaction, options).await;
		}
		"ungrouped" => {
			view_ungrouped(context, interaction).await;
		}
		"reposition" => {
			// let _ = ephemeral_reply(context, interaction, "Not yet implemented.").await;
			// return;
			reposition(context, interaction, options).await;
		}
		_ => panic!("Received invalid subcommand name."),
	}
}

async fn add(
	context: Context<'_>,
	interaction: CommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let group_name = options
		.get(0)
		.and_then(|option| option.value.as_str())
		.unwrap();

	let emojis = match get_and_parse_emoji_option(context.emoji_map, options.get(1)) {
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
	interaction: CommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let subcommand = options.get(0).unwrap();
	let CommandDataOptionValue::SubCommand(ref options) = subcommand.value else {
		panic!();
	};
	let (group, emojis) = if subcommand.name.as_str() == "from" {
		let group_name = options
			.get(0)
			.and_then(|option| option.value.as_str())
			.unwrap();
		(Some(group_name), options.get(1))
	} else {
		(None, options.get(0))
	};

	let emojis = match get_and_parse_emoji_option(context.emoji_map, emojis) {
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
	interaction: CommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let group = options
		.get(0)
		.and_then(|option| option.value.as_str())
		.unwrap();
	let new_name = options
		.get(1)
		.and_then(|option| option.value.as_str())
		.unwrap();

	let old_name = match rename_group(context.database, interaction.user.id, group, new_name).await
	{
		Ok(old_name) => old_name,
		Err(RenameGroupError::NoSuchGroup) => {
			let _ = interaction
				.ephemeral_reply(
					context.http,
					format!("You have no group called \"{group}\"."),
				)
				.await;
			return;
		}
		Err(RenameGroupError::NameTaken(taken_name)) => {
			let _ = interaction
				.ephemeral_reply(
					context.http,
					format!("There is already a group named \"{taken_name}\"."),
				)
				.await;
			return;
		}
	};

	let message = format!("Renamed group {} to {}.", old_name, new_name);
	let _ = interaction.ephemeral_reply(context.http, message).await;
}

async fn list(context: Context<'_>, interaction: CommandInteraction) {
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
	interaction: CommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let group = options
		.get(0)
		.and_then(|option| option.value.as_str())
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

async fn view_ungrouped(context: Context<'_>, interaction: CommandInteraction) {
	let emojis =
		get_ungrouped_emojis(context.database, context.emoji_map, interaction.user.id).await;

	let message = if emojis.is_empty() {
		format!("You have no ungrouped emojis.")
	} else {
		format!("Ungrouped emojis: {}", emojis)
	};
	_ = interaction.ephemeral_reply(context.http, message).await;
}

async fn reposition(
	context: Context<'_>,
	interaction: CommandInteraction,
	options: Vec<CommandDataOption>,
) {
	let group = options
		.get(0)
		.and_then(|option| option.value.as_str())
		.unwrap();
	let Some::<u32>(new_position) = options
		.get(1)
		.and_then(|option| option.value.as_i64())
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

#[rustfmt::skip]
pub fn register() -> CreateCommand {
	CreateCommand::new("group").description("Interact with emoji groups.")
		.add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "add", "Add emojis to a group. Makes the group if it doesn't already exist.")
			.add_sub_option(CreateCommandOption::new(CommandOptionType::String, "group", "The group to add the emojis to.").max_length(50).required(true))
			.add_sub_option(CreateCommandOption::new(CommandOptionType::String, "emojis", "The emojis to add to the group.").required(true))
		)
		.add_option(CreateCommandOption::new(CommandOptionType::SubCommandGroup, "remove", "Removes emojis from a group.")
			.add_sub_option(CreateCommandOption::new(CommandOptionType::SubCommand, "from", "Removes emojis from a specific group.")
				.add_sub_option(CreateCommandOption::new(CommandOptionType::String, "group", "The group to remove the emojis from.").max_length(50).required(true))
				.add_sub_option(CreateCommandOption::new(CommandOptionType::String, "emojis", "The emojis to remove from the group.").required(true))
			)
			.add_sub_option(CreateCommandOption::new(CommandOptionType::SubCommand, "emojis", "Removes emojis from whatever group.")
				.add_sub_option(CreateCommandOption::new(CommandOptionType::String, "emojis", "The emojis to remove from whatever group they're in.").required(true))
			)
		)
		.add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "rename", "Renames an emoji group.")
			.add_sub_option(CreateCommandOption::new(CommandOptionType::String, "group", "The group to rename.").max_length(50).required(true))
			.add_sub_option(CreateCommandOption::new(CommandOptionType::String, "new_name", "The new name for the group.").max_length(50).required(true))
		)
		.add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "list", "Lists all your emoji groups."))
		.add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "view", "Views the contents of one of your emoji groups.")
			.add_sub_option(CreateCommandOption::new(CommandOptionType::String, "group", "The group to view the contents of.").max_length(50).required(true))
		)
		.add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "ungrouped", "Views the ungrouped emojis."))
		.add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "reposition", "Repositions the group in the group list. This is mostly relevant when viewing inventory.")
			.add_sub_option(CreateCommandOption::new(CommandOptionType::String, "group", "The group to reposition.").max_length(50).required(true))
			.add_sub_option(CreateCommandOption::new(CommandOptionType::Integer, "position", "The position to move the group to, where 0 is the first.").min_int_value(0).required(true))
		)
}
