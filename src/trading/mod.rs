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
	emojis::EmojiMap,
	util::{get_name, parse_emoji_input},
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
	let offer = parse_emoji_input(
		emoji_map,
		options
			.get(1)
			.and_then(|option| option.value.as_ref())
			.and_then(|value| value.as_str())
			.unwrap(),
	)?;
	if offer.is_empty() {
		return Err(String::from("Offer is empty."));
	}
	let request = parse_emoji_input(
		emoji_map,
		options
			.get(2)
			.and_then(|option| option.value.as_ref())
			.and_then(|value| value.as_str())
			.unwrap(),
	)?;
	if request.is_empty() {
		return Err(String::from("Request is empty."));
	}
	let trade_offer = TradeOffer::new(user, target_user, offer, request)?;
	if !does_user_have_emotes(executor, user, trade_offer.offer()).await {
		return Err(String::from("You don't have those emojis to offer."));
	}

	let mut output = String::from("You are now offering ");
	trade_offer.write_offer(&mut output);
	let name = get_name(context, guild, target_user).await;
	write!(output, " in return for {}'s ", name).unwrap();
	trade_offer.write_request(&mut output);
	output.push('.');

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
			output.push_str("You are offering ");
			trade.write_offer(&mut output);
			let name = get_name(context, guild, trade.target_user()).await;
			write!(output, " for {name}'s ").unwrap();
			trade.write_request(&mut output);
			output.push_str(".\n");
		}
	}
	if !incoming.is_empty() {
		output.push_str("Incoming:\n");
		for trade in incoming {
			let name = get_name(context, guild, trade.offering_user()).await;
			write!(output, "{name} is offering ").unwrap();
			trade.write_offer(&mut output);
			output.push_str(" for your ");
			trade.write_request(&mut output);
			output.push_str(".\n");
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

	let s = if trade.request().len() != 1 { "s" } else { "" };
	let mut content = format!("You are about to accept the trade offer from {offerer_name}.\nYou will **lose** the following emoji{s}: ");
	trade.write_request(&mut content);
	let s = if trade.offer().len() != 1 { "s" } else { "" };
	write!(content, "\nYou will **gain** the following emoji{s}: ").unwrap();
	trade.write_offer(&mut content);
	content.push_str("\nDo you want to proceed?");

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
				let result = try_confirm_trade(executor, emoji_map, trade, offerer_name, accepter_name).await;
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
	if !does_user_have_emotes(executor, target_user, trade.request()).await {
		return TradeOfferValidation::TargetLacksEmojis;
	}
	if !does_user_have_emotes(executor, offering_user, trade.offer()).await {
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
	let mut output = String::new();
	write!(output, "{accepter_name} successfully traded away ").unwrap();
	trade.write_request(&mut output);
	write!(output, " to {offerer_name} in exchange for ").unwrap();
	trade.write_offer(&mut output);
	output.push('.');
	Ok(output)
}
