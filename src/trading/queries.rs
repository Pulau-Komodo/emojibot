use serenity::model::prelude::UserId;
use sqlx::{query, Executor, Pool, Sqlite, Transaction};

use crate::emojis::{Emoji, EmojiMap};

use super::trade_offer::TradeOffer;

pub(super) async fn add_trade_offer(executor: &Pool<Sqlite>, trade_offer: TradeOffer) {
	let user_id = trade_offer.offering_user().0 as i64;
	let target_user_id = trade_offer.target_user().0 as i64;
	let emojis = trade_offer.flatten();

	let mut transaction = executor.begin().await.unwrap();
	let trade_id = query!(
		"
		INSERT INTO
			trade_offers (user, target_user)
		VALUES
			(?, ?)
		",
		user_id,
		target_user_id
	)
	.execute(&mut *transaction)
	.await
	.unwrap()
	.last_insert_rowid();
	for (emoji, count) in emojis {
		let emoji = emoji.as_str();
		query!(
			"
			INSERT INTO
				trade_offer_contents (trade, emoji, count)
			VALUES
				(?, ?, ?)
			",
			trade_id,
			emoji,
			count
		)
		.execute(&mut *transaction)
		.await
		.unwrap();
	}
	transaction.commit().await.unwrap();
}

pub(super) async fn remove_trade_offer<'c, E>(executor: E, user: UserId, target_user: UserId)
where
	E: Executor<'c, Database = Sqlite>,
{
	let user_id = user.0 as i64;
	let target_user_id = target_user.0 as i64;
	query!(
		"
		DELETE FROM
			trade_offers
		WHERE
			user = ? AND target_user = ?
		",
		user_id,
		target_user_id
	)
	.execute(executor)
	.await
	.unwrap();
}

pub(super) async fn get_trade_emojis(
	executor: &mut Transaction<'_, Sqlite>,
	emoji_map: &EmojiMap,
	trade: i64,
) -> Vec<(Emoji, i64)> {
	query!(
		"
		SELECT
			emoji, count
		FROM
			trade_offer_contents
		WHERE
			trade = ?
		",
		trade
	)
	.fetch_all(&mut **executor)
	.await
	.unwrap()
	.into_iter()
	.map(|record| {
		let emoji = *emoji_map
			.get(record.emoji.as_str())
			.expect("Could not find emoji from database in emoji map.");
		(emoji, record.count)
	})
	.collect::<Vec<_>>()
}

pub(super) async fn get_outgoing_trade_offers(
	executor: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	user: UserId,
) -> Vec<TradeOffer> {
	let user_id = user.0 as i64;
	let mut transaction = executor.begin().await.unwrap();
	let offers = query!(
		"
		SELECT
			id, target_user
		FROM
			trade_offers
		WHERE
			user = ?
		",
		user_id
	)
	.fetch_all(&mut *transaction)
	.await
	.unwrap();
	let mut full_offers = Vec::new();
	for record in offers {
		let emojis = get_trade_emojis(&mut transaction, emoji_map, record.id).await;
		let offer = TradeOffer::from_database(user, UserId(record.target_user as u64), emojis);
		full_offers.push(offer);
	}
	transaction.commit().await.unwrap();
	full_offers
}

pub(super) async fn get_incoming_trade_offers(
	executor: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	user: UserId,
) -> Vec<TradeOffer> {
	let user_id = user.0 as i64;
	let mut transaction = executor.begin().await.unwrap();
	let offers = query!(
		"
		SELECT
			id, user
		FROM
			trade_offers
		WHERE
			target_user = ?
		",
		user_id
	)
	.fetch_all(&mut *transaction)
	.await
	.unwrap();
	let mut full_offers = Vec::new();
	for record in offers {
		let emojis = get_trade_emojis(&mut transaction, emoji_map, record.id).await;
		let offer = TradeOffer::from_database(UserId(record.user as u64), user, emojis);
		full_offers.push(offer);
	}
	transaction.commit().await.unwrap();
	full_offers
}

pub(super) async fn get_trade_offer(
	executor: &Pool<Sqlite>,
	emoji_map: &EmojiMap,
	offering_user: UserId,
	target_user: UserId,
) -> Option<TradeOffer> {
	let offering_user_id = offering_user.0 as i64;
	let target_user_id = target_user.0 as i64;
	let mut transaction = executor.begin().await.unwrap();
	let offer = query!(
		"
		SELECT
			id
		FROM
			trade_offers
		WHERE
			user = ? AND target_user = ?
		",
		offering_user_id,
		target_user_id
	)
	.fetch_optional(&mut *transaction)
	.await
	.unwrap()?;
	let emojis = get_trade_emojis(&mut transaction, emoji_map, offer.id).await;
	transaction.commit().await.unwrap();
	Some(TradeOffer::from_database(
		offering_user,
		target_user,
		emojis,
	))
}

/// Check a user's emoji inventory to see if it has the emojis for a trade.
pub(super) async fn does_user_have_emotes(
	executor: &Pool<Sqlite>,
	user: UserId,
	emojis: &[(Emoji, i64)],
) -> bool {
	let user_id = user.0 as i64;
	let mut transaction = executor.begin().await.unwrap();
	for (emoji, target_count) in emojis {
		let emoji = emoji.as_str();
		let count = query!(
			"
				SELECT
					count
				FROM
					emoji_inventory
				WHERE
					user = ? AND emoji = ?
				",
			user_id,
			emoji
		)
		.fetch_optional(&mut *transaction)
		.await
		.unwrap()
		.map(|record| record.count)
		.unwrap_or(0);
		if count < *target_count {
			transaction.commit().await.unwrap();
			return false;
		}
	}
	transaction.commit().await.unwrap();
	true
}

pub(super) async fn does_trade_offer_exist(
	executor: &Pool<Sqlite>,
	user: UserId,
	target_user: UserId,
) -> bool {
	let user_id = user.0 as i64;
	let target_user_id = target_user.0 as i64;
	query!(
		"
		SELECT
			COUNT() as count
		FROM
			trade_offers
		WHERE
			user = ? AND target_user = ?
		",
		user_id,
		target_user_id
	)
	.fetch_one(executor)
	.await
	.unwrap()
	.count != 0
}

pub(super) async fn complete_trade(executor: &Pool<Sqlite>, trade_offer: &TradeOffer) {
	let mut transaction = executor.begin().await.unwrap();

	log_trade(&mut transaction, trade_offer).await;
	
	remove_trade_offer(
		&mut *transaction,
		trade_offer.offering_user(),
		trade_offer.target_user(),
	)
	.await;

	for (emoji, count) in trade_offer.offer() {
		transfer_emoji(&mut transaction, *emoji, *count, trade_offer.offering_user(), trade_offer.target_user()).await;
	}
	for (emoji, count) in trade_offer.request() {
		transfer_emoji(&mut transaction, *emoji, *count, trade_offer.target_user(), trade_offer.offering_user()).await;
	}

	// Clean up count = 0 emojis
	query!(
		"
		DELETE FROM
			emoji_inventory
		WHERE
			count = 0
		"
	)
	.execute(&mut *transaction)
	.await
	.unwrap();

	transaction.commit().await.unwrap();
}

async fn log_trade(executor: &mut Transaction<'_, Sqlite>, trade_offer: &TradeOffer) {
	let offering_user_id = trade_offer.offering_user().0 as i64;
	let target_user = trade_offer.target_user().0 as i64;
	let id = query!(
		"
		INSERT INTO
			trade_log (initiating_user, recipient_user)
		VALUES
			(?, ?)
		",
		offering_user_id,
		target_user,
	)
	.execute(&mut **executor)
	.await
	.unwrap()
	.last_insert_rowid();
	for (emoji, count) in trade_offer.flatten() {
		let emoji = emoji.as_str();
		query!(
			"
			INSERT INTO
				trade_log_contents (trade, emoji, count)
			VALUES
				(?, ?, ?)
			",
			id,
			emoji,
			count
		)
		.execute(&mut **executor)
		.await
		.unwrap();
	}
}

async fn transfer_emoji(
	transaction: &mut Transaction<'_, Sqlite>,
	emoji: Emoji,
	count: i64,
	from: UserId,
	to: UserId,
) {
	let emoji = emoji.as_str();
	let from_id = from.0 as i64;
	let to_id = to.0 as i64;
	query!(
		"
		UPDATE
			emoji_inventory
		SET
			count = count - ?
		WHERE
			user = ? AND emoji = ?
		",
		count,
		from_id,
		emoji
	)
	.execute(&mut **transaction)
	.await
	.unwrap();
	query!(
		"
		INSERT INTO
			emoji_inventory (user, emoji, count)
		VALUES
			(?, ?, ?)
		ON CONFLICT DO UPDATE SET
			count = count + excluded.count
		",
		to_id,
		emoji,
		count
	)
	.execute(&mut **transaction)
	.await
	.unwrap();
}
