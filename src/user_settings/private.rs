use serenity::{
	all::{CommandInteraction, UserId},
	builder::CreateCommand,
};
use sqlx::{query, Pool, Sqlite};

use crate::{context::Context, util::ReplyShortcuts};

pub async fn is_private(executor: &Pool<Sqlite>, user: UserId) -> bool {
	let user_id = user.get() as i64;
	query!(
		"
		SELECT private
		FROM user_settings
		WHERE user = ?
		",
		user_id
	)
	.fetch_optional(executor)
	.await
	.unwrap()
	.is_some_and(|record| record.private != 0)
}

/// Toggles a user's private setting, and returns whether it is now private or not.
async fn toggle_private(executor: &Pool<Sqlite>, user: UserId) -> bool {
	let user_id = user.get() as i64;
	query!(
		"
		INSERT INTO user_settings (user, private)
		VALUES (?, 1)
		ON CONFLICT (user)
			DO UPDATE SET private = 1 - private
		RETURNING private
		",
		user_id
	)
	.fetch_one(executor)
	.await
	.unwrap()
	.private
		!= 0
}

pub async fn execute(context: Context<'_>, interaction: CommandInteraction) {
	let is_private = toggle_private(context.database, interaction.user.id).await;
	let content = if is_private {
		"Your emoji inventory was set to private. Others can no longer view your emoji inventory or find emojis in your inventory, recycling input and outcome will be private, and you won't be notified of new emojis through reactions."
	} else {
		"Your emoji inventory was set to public. Others can now view your emoji inventory and find emojis in your inventory, recycling input and outcome will be public, and you will be notified of new emojis through reactions."
	};
	let _ = interaction.ephemeral_reply(context.http, content).await;
}

pub fn register() -> CreateCommand {
	CreateCommand::new("private")
		.description("Toggle whether your emoji inventory should be private.")
}
