pub(crate) mod command;
mod queries;
mod trade_offer;

use serenity::{
	builder::CreateComponents,
	model::prelude::{
		application_command::{ApplicationCommandInteraction, CommandDataOption},
		component::ButtonStyle,
		GuildId, InteractionResponseType, UserId,
	},
	prelude::Context,
};
use sqlx::{Pool, Sqlite};
use std::fmt::Write;

use crate::{
	emoji::EmojiMap,
	emojis_with_counts::EmojisWithCounts,
	util::{get_and_parse_emoji_option, get_name},
};

use self::{queries::*, trade_offer::TradeOffer};

pub(super) async fn try_offer_trade(
	executor: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: &Context,
	options: Vec<CommandDataOption>,
	guild: GuildId,
	user: UserId,
	target_user: UserId,
) -> Result<String, String> {
	if user == target_user {
		return Err(String::from("You can't trade yourself."));
	}
	if does_trade_offer_exist(executor, user, target_user).await {
		return Err(String::from("You already have a trade offer to that user."));
	}
	let offer = get_and_parse_emoji_option(emoji_map, &options, 1)?;
	if offer.is_empty() {
		return Err(String::from("Offer is empty."));
	}
	let request = get_and_parse_emoji_option(emoji_map, &options, 2)?;
	if request.is_empty() {
		return Err(String::from("Request is empty."));
	}
	let offer = EmojisWithCounts::from_flat(&offer);
	let request = EmojisWithCounts::from_flat(&request);
	let trade_offer = TradeOffer::new(user, target_user, offer, request)?;
	if !trade_offer.offer().are_owned_by_user(executor, user).await {
		return Err(String::from("You don't have those emojis to offer."));
	}

	let name = get_name(context, guild, target_user).await;
	let output = format!(
		"You are now offering {} in return for {}'s {}.",
		trade_offer.offer(),
		name,
		trade_offer.request()
	);

	add_trade_offer(executor, trade_offer).await;

	Ok(output)
}

pub(super) async fn try_cancel_offer(
	executor: &Pool<Sqlite>,
	context: &Context,
	guild: GuildId,
	user: UserId,
	target_user: UserId,
) -> Result<String, String> {
	let name = get_name(context, guild, target_user).await;
	if !does_trade_offer_exist(executor, user, target_user).await {
		return Err(format!("You have no trade offer to {}.", name));
	}

	remove_trade_offer(executor, user, target_user).await;

	Ok(format!("Trade offer to {} rescinded.", name))
}

pub(super) async fn try_reject_offer(
	executor: &Pool<Sqlite>,
	context: &Context,
	guild: GuildId,
	user: UserId,
	other_user: UserId,
) -> Result<String, String> {
	let name = get_name(context, guild, other_user).await;
	if !does_trade_offer_exist(executor, other_user, user).await {
		return Err(format!("You have no trade offer from {}.", name));
	}

	remove_trade_offer(executor, other_user, user).await;

	Ok(format!("Trade offer from {} rejected.", name))
}

pub(super) async fn view_offers(
	executor: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: &Context,
	guild: GuildId,
	user: UserId,
) -> Result<String, String> {
	let outgoing = get_outgoing_trade_offers(executor, emoji_map, user).await;
	let incoming = get_incoming_trade_offers(executor, emoji_map, user).await;

	let mut output = String::new();
	if !outgoing.is_empty() {
		output.push_str("Outgoing:\n");
		for trade in outgoing {
			let name = get_name(context, guild, trade.target_user()).await;
			output
				.write_fmt(format_args!(
					"You are offering {} for {}'s {}.\n",
					trade.offer(),
					name,
					trade.request()
				))
				.unwrap();
		}
	}
	if !incoming.is_empty() {
		output.push_str("Incoming:\n");
		for trade in incoming {
			let name = get_name(context, guild, trade.offering_user()).await;
			output
				.write_fmt(format_args!(
					"{} is offering {} for your {}.\n",
					name,
					trade.offer(),
					trade.request()
				))
				.unwrap();
		}
	}
	if output.is_empty() {
		output.push_str("You have no outgoing or incoming trade offers.");
	}
	Ok(output)
}

pub(super) async fn try_accept_offer(
	executor: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: &Context,
	interaction: &ApplicationCommandInteraction,
	guild: GuildId,
	accepting_user: UserId,
	offering_user: UserId,
) -> Result<(), String> {
	let offerer_name = get_name(context, guild, offering_user).await;
	let trade = match validate_trade_offer(executor, emoji_map, offering_user, accepting_user).await
	{
		TradeOfferValidation::NoTrade => Err(format!(
			"You do not have a trade offer from {offerer_name}."
		)),
		TradeOfferValidation::TargetLacksEmojis => {
			Err(String::from("You do not have the requested emojis."))
		}
		TradeOfferValidation::OffererLacksEmojis => Err(format!(
			"Something went wrong: {offerer_name} does not have the offered emojis."
		)),
		TradeOfferValidation::Valid(trade) => Ok(trade),
	}?;

	let s1 = if trade.request().emoji_count() != 1 {
		"s"
	} else {
		""
	};
	let s2 = if trade.offer().emoji_count() != 1 {
		"s"
	} else {
		""
	};
	let content = format!("You are about to accept the trade offer from {offerer_name}.\nYou will **lose** the following emoji{s1}: {}\nYou will **gain** the following emoji{s2}: {}nDo you want to proceed?", trade.request(), trade.offer());

	let _ = interaction
		.create_interaction_response(&context.http, |interaction| {
			interaction
				.kind(InteractionResponseType::ChannelMessageWithSource)
				.interaction_response_data(|data| {
					data.content(content)
						.ephemeral(true)
						.components(|component| {
							component.create_action_row(|row| {
								row.create_button(|button| {
									button
										.label("Yes")
										.style(ButtonStyle::Primary)
										.custom_id("yes")
								})
								.create_button(|button| {
									button
										.label("No")
										.style(ButtonStyle::Secondary)
										.custom_id("no")
								})
							})
						})
				})
		})
		.await;

	let message = interaction
		.get_interaction_response(&context.http)
		.await
		.map_err(|_| String::from("Error retrieving interaction response."))?;
	let button_press = message
		.await_component_interaction(context)
		.collect_limit(1)
		.timeout(std::time::Duration::from_secs(60))
		.await;

	if let Some(button_press) = button_press {
		match button_press.data.custom_id.as_str() {
			"yes" => {
				let accepter_name = get_name(context, guild, accepting_user).await;
				let result =
					try_confirm_trade(executor, emoji_map, trade, offerer_name, accepter_name)
						.await;
				match result {
					Ok(content) => {
						let _ = button_press
							.create_interaction_response(&context.http, |response| {
								response
									.kind(InteractionResponseType::ChannelMessageWithSource)
									.interaction_response_data(|data| {
										data.content(content).ephemeral(false)
									})
							})
							.await;
					}
					Err(content) => {
						let _ = button_press
							.create_interaction_response(&context.http, |response| {
								response
									.kind(InteractionResponseType::ChannelMessageWithSource)
									.interaction_response_data(|data| {
										data.content(content).ephemeral(true)
									})
							})
							.await;
					}
				}

				let _ = interaction
					.delete_original_interaction_response(&context.http)
					.await;
			}
			"no" => {
				let _ = button_press
					.create_interaction_response(&context.http, |response| {
						response
							.kind(InteractionResponseType::UpdateMessage)
							.interaction_response_data(|data| {
								data.content("You have cancelled the trade.")
									.set_components(CreateComponents::default())
							})
					})
					.await;
			}
			_ => panic!(),
		}
	} else {
		let _ = interaction
			.create_followup_message(&context.http, |response| {
				response
					.content("The trade confirmation has timed out.")
					.ephemeral(true)
			})
			.await;
		let _ = interaction
			.delete_original_interaction_response(&context.http)
			.await;
	}
	Ok(())
}

enum TradeOfferValidation {
	Valid(TradeOffer),
	NoTrade,
	TargetLacksEmojis,
	OffererLacksEmojis,
}

async fn validate_trade_offer(
	executor: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	offering_user: UserId,
	target_user: UserId,
) -> TradeOfferValidation {
	let Some(trade) = get_trade_offer(
		executor,
		emoji_map,
		offering_user,
		target_user,
	)
	.await else {
		return TradeOfferValidation::NoTrade;
	};
	if !trade
		.request()
		.are_owned_by_user(executor, target_user)
		.await
	{
		return TradeOfferValidation::TargetLacksEmojis;
	}
	if !trade
		.offer()
		.are_owned_by_user(executor, offering_user)
		.await
	{
		return TradeOfferValidation::OffererLacksEmojis;
	}
	TradeOfferValidation::Valid(trade)
}

async fn try_confirm_trade(
	executor: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	trade_offer: TradeOffer,
	offerer_name: String,
	accepter_name: String,
) -> Result<String, String> {
	let trade = match validate_trade_offer(
		executor,
		emoji_map,
		trade_offer.offering_user(),
		trade_offer.target_user(),
	)
	.await
	{
		TradeOfferValidation::NoTrade => Err(format!(
			"The trade offer from {offerer_name} is no longer there."
		)),
		TradeOfferValidation::TargetLacksEmojis => {
			Err(String::from("You no longer have the requested emojis."))
		}
		TradeOfferValidation::OffererLacksEmojis => {
			Err(format!("{offerer_name} no longer has the offered emojis."))
		}
		TradeOfferValidation::Valid(trade) => Ok(trade),
	}?;
	if trade_offer != trade {
		return Err(format!(
			"The offer from {offerer_name} was changed while you were accepting it, so the trade was cancelled."
		));
	}

	complete_trade(executor, &trade_offer).await;
	remove_invalidated_trade_offers(executor, &trade_offer).await;

	let output = format!(
		"{accepter_name} successfully traded away {} to {offerer_name} in exchange for {}.",
		trade.request(),
		trade.offer()
	);
	Ok(output)
}
