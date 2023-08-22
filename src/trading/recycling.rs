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
	emojis_with_counts::EmojisWithCounts,
	inventory::queries::remove_empty_groups,
	queries::give_emoji,
	trading::{queries::log_trade, trade_offer::TradeOffer},
	user_settings::private::is_private,
	util::{ephemeral_reply, get_name, parse_emoji_input, public_reply},
};

use super::queries::remove_invalidated_trade_offers;

async fn recycle(database: &Pool<Sqlite>, user: UserId, emojis: EmojisWithCounts) -> Emoji {
	let user_id = user.0 as i64;
	let trade_offer = TradeOffer::new_recycling(user, emojis);
	let random_emoji = trade_offer.recycling_emoji();

	let mut transaction = database.begin().await.unwrap();

	log_trade(&mut transaction, &trade_offer).await;

	for (emoji, count) in trade_offer.offer() {
		let emoji_str = emoji.as_str();
		let rows_affected = query!(
			"
			DELETE FROM emoji_inventory
			WHERE user = ? AND emoji = ? AND rowid IN (
				SELECT emoji_inventory.rowid
				FROM emoji_inventory
				LEFT JOIN emoji_inventory_groups
				ON emoji_inventory.group_id = emoji_inventory_groups.id
				WHERE emoji_inventory.user = ? AND emoji_inventory.emoji = ?
				ORDER BY sort_order DESC
				LIMIT ?
			)
			",
			user_id,
			emoji_str,
			user_id,
			emoji_str,
			count
		)
		.execute(&mut *transaction)
		.await
		.unwrap()
		.rows_affected();

		if rows_affected != *count as u64 {
			transaction.rollback().await.unwrap();
			panic!("Wrong number of rows affected on a recycle: {rows_affected} != {count}.");
		}
	}

	give_emoji(&mut *transaction, user, random_emoji).await;

	remove_empty_groups(&mut transaction, user).await;

	transaction.commit().await.unwrap();

	remove_invalidated_trade_offers(database, &trade_offer).await;

	random_emoji
}

pub async fn execute(
	database: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	context: Context,
	interaction: ApplicationCommandInteraction,
) {
	let input = interaction
		.data
		.options
		.first()
		.and_then(|option| option.value.as_ref())
		.and_then(|value| value.as_str())
		.unwrap();
	let emojis = match parse_emoji_input(emoji_map, input) {
		Ok(emojis) => emojis,
		Err(message) => {
			let _ = ephemeral_reply(context, interaction, message).await;
			return;
		}
	};
	if emojis.len() != 3 {
		let _ = ephemeral_reply(context, interaction, "You must specify exactly 3 emojis.").await;
		return;
	}
	let emojis = EmojisWithCounts::from_flat(&emojis);
	if !emojis
		.are_owned_by_user(database, interaction.user.id)
		.await
	{
		let _ = ephemeral_reply(context, interaction, "You don't own all specified emojis.").await;
		return;
	}

	let emoji = recycle(database, interaction.user.id, emojis.clone()).await;

	if is_private(database, interaction.user.id).await {
		let message = format!("You recycled {emojis} and got {emoji}.");
		let _ = ephemeral_reply(context, interaction, message).await;
	} else {
		let name = get_name(&context, interaction.guild_id.unwrap(), interaction.user.id).await;
		let message = format!("{name} recycled {emojis} and got {emoji}.");
		let _ = public_reply(context, interaction, message).await;
	}
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
	command
		.name("recycle")
		.description("Recycle 3 emojis for a new one.")
		.create_option(|option| {
			option
				.name("emojis")
				.description("The 3 emojis to recycle.")
				.kind(CommandOptionType::String)
				.required(true)
		})
}
