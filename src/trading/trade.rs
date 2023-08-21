use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{
		application_command::ApplicationCommandInteraction, command::CommandOptionType, UserId,
	},
	prelude::Context,
};
use sqlx::{Pool, Sqlite};

use crate::{emoji::EmojiMap, util::interaction_reply};

use super::{try_accept_offer, try_cancel_offer, try_offer_trade, try_reject_offer, view_offers};

pub async fn execute(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: Context,
	mut interaction: ApplicationCommandInteraction,
) {
	let subcommand = interaction.data.options.pop().unwrap();
	let argument_user = subcommand.options.get(0).and_then(|option| {
		option
			.value
			.as_ref()
			.map(|value| UserId(value.as_str().unwrap().parse().unwrap()))
	});
	let user = interaction.user.id;
	let guild = interaction.guild_id.unwrap();

	let mut ephemeral = false;
	let result = match subcommand.name.as_str() {
		"offer" => {
			try_offer_trade(
				database,
				emoji_map,
				&context,
				subcommand.options,
				guild,
				user,
				argument_user.unwrap(),
			)
			.await
		}
		"withdraw" => {
			try_cancel_offer(database, &context, guild, user, argument_user.unwrap()).await
		}
		"accept" => {
			let result = try_accept_offer(
				database,
				emoji_map,
				&context,
				&interaction,
				guild,
				user,
				argument_user.unwrap(),
			)
			.await;
			if let Err(result) = result {
				Err(result)
			} else {
				return;
			}
		}
		"reject" => try_reject_offer(database, &context, guild, user, argument_user.unwrap()).await,
		"view" => {
			ephemeral = true;
			view_offers(database, emoji_map, &context, guild, user).await
		}
		_ => panic!("Received an invalid interaction subcommand."),
	};
	let _ = match result {
		Ok(message) => interaction_reply(context, interaction, message, ephemeral).await,
		Err(error) => interaction_reply(context, interaction, error, true).await,
	};
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
	command
		.name("trade")
		.description("Make, withdraw, accept or reject a trade offer, or view trade offers.")
		.create_option(|option| {
			option
				.name("offer")
				.description("Offer a trade to a user.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("user")
						.description("Whom the trade offer is to.")
						.kind(CommandOptionType::User)
						.required(true)
				})
				.create_sub_option(|option| {
					option.name("offer")
						.description("The emojis you are offering in this trade. Repeat emojis for multiples.")
						.kind(CommandOptionType::String)
						.required(true)
				})
				.create_sub_option(|option| {
					option
						.name("request")
						.description(
							"The emojis requested in this trade. Repeat emojis for multiples.",
						)
						.kind(CommandOptionType::String)
						.required(true)
				})
		})
		.create_option(|option| {
			option
				.name("withdraw")
				.description("Withdraw a trade offer to a user.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("user")
						.description("Whom the trade offer is to.")
						.kind(CommandOptionType::User)
						.required(true)
				})
		})
		.create_option(|option| {
			option
				.name("accept")
				.description("Accept a trade offer from a user.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("user")
						.description("Whose trade offer to you to accept.")
						.kind(CommandOptionType::User)
						.required(true)
				})
		})
		.create_option(|option| {
			option
				.name("reject")
				.description("Reject a trade offer from a user.")
				.kind(CommandOptionType::SubCommand)
				.create_sub_option(|option| {
					option
						.name("user")
						.description("Whose trade offer for you to reject.")
						.kind(CommandOptionType::User)
						.required(true)
				})
		})
		.create_option(|option| {
			option
				.name("view")
				.description("View incoming and outgoing trade offers.")
				.kind(CommandOptionType::SubCommand)
		})
}
