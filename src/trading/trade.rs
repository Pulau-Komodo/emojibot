use serenity::{
	all::{CommandDataOptionValue, CommandInteraction, CommandOptionType},
	builder::{CreateCommand, CreateCommandOption},
	gateway::ShardMessenger,
};

use crate::{context::Context, util::ReplyShortcuts};

use super::{try_accept_offer, try_cancel_offer, try_offer_trade, try_reject_offer, view_offers};

pub async fn execute(
	context: Context<'_>,
	shard_messenger: ShardMessenger,
	mut interaction: CommandInteraction,
) {
	let subcommand = interaction.data.options.pop().unwrap();
	let CommandDataOptionValue::SubCommand(options) = subcommand.value else {
		panic!()
	};
	let argument_user = options.get(0).and_then(|option| option.value.as_user_id());
	let user = interaction.user.id;
	let guild = interaction.guild_id.unwrap();

	let mut ephemeral = false;
	let result = match subcommand.name.as_str() {
		"offer" => try_offer_trade(context, options, guild, user, argument_user.unwrap()).await,
		"withdraw" => try_cancel_offer(context, guild, user, argument_user.unwrap()).await,
		"accept" => {
			let result = try_accept_offer(
				context,
				shard_messenger,
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
		"reject" => try_reject_offer(context, guild, user, argument_user.unwrap()).await,
		"view" => {
			ephemeral = true;
			view_offers(context, guild, user).await
		}
		_ => panic!("Received an invalid interaction subcommand."),
	};
	let _ = match result {
		Ok(message) => interaction.reply(context.http, message, ephemeral).await,
		Err(error) => interaction.ephemeral_reply(context.http, error).await,
	};
}

pub fn register() -> CreateCommand {
	CreateCommand::new("trade")
		.description("Make, withdraw, accept or reject a trade offer, or view trade offers.")
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::SubCommand,
				"offer",
				"Offer a trade to a user.",
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::User,
					"user",
					"Whom the trade offer is to.",
				)
				.required(true),
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::String,
					"offer",
					"The emojis you are offering in this trade. Repeat emojis for multiples.",
				)
				.required(true),
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::String,
					"request",
					"The emojis requested in this trade. Repeat emojis for multiples.",
				)
				.required(true),
			),
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::SubCommand,
				"withdraw",
				"Withdraw a trade offer to a user.",
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::User,
					"user",
					"Whom the trade offer is to.",
				)
				.required(true),
			),
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::SubCommand,
				"accept",
				"Accept a trade offer from a user.",
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::User,
					"user",
					"Whose trade offer to you to accept.",
				)
				.required(true),
			),
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::SubCommand,
				"reject",
				"Reject a trade offer from a user.",
			)
			.add_sub_option(
				CreateCommandOption::new(
					CommandOptionType::User,
					"user",
					"Whose trade offer for you to reject.",
				)
				.required(true),
			),
		)
		.add_option(CreateCommandOption::new(
			CommandOptionType::SubCommand,
			"view",
			"View incoming and outgoing trade offers.",
		))
}
