use itertools::Itertools;
use serenity::{async_trait, model::prelude::*, prelude::*};
use sqlx::{Pool, Sqlite};

use crate::{
	daily_emoji::maybe_give_daily_emoji, emoji::EmojiMap, find_emoji, images, inventory, trading,
	user_settings,
};

pub struct DiscordEventHandler {
	database: Pool<Sqlite>,
	emoji_map: EmojiMap,
}

impl DiscordEventHandler {
	pub fn new(database: Pool<Sqlite>, emoji_map: EmojiMap) -> Self {
		Self {
			database,
			emoji_map,
		}
	}
}

#[async_trait]
impl EventHandler for DiscordEventHandler {
	async fn message(&self, context: Context, message: Message) {
		if !message.is_own(&context.cache) && !message.author.bot {
			maybe_give_daily_emoji(&self.database, context, message).await;
		}
	}

	async fn interaction_create(&self, context: Context, interaction: Interaction) {
		let shard_manager = context.shard;
		if let Interaction::Command(interaction) = interaction {
			let context = crate::context::Context::new(
				&self.database,
				&self.emoji_map,
				&context.http,
				&context.cache,
			);

			match interaction.data.name.as_str() {
				"inventory" => inventory::view::execute(context, interaction).await,
				"group" => inventory::group::execute(context, interaction).await,
				"who" => find_emoji::execute(context, interaction).await,
				"trade" => trading::trade::execute(context, shard_manager, interaction).await,
				"recycle" => trading::recycling::execute(context, interaction).await,
				"private" => user_settings::private::execute(context, interaction).await,
				"image" => images::rasterize::execute(context, interaction).await,
				"generate" => images::generate::execute(context, interaction).await,
				"testimage" => images::generate::execute_test(context, interaction).await,
				_ => (),
			};
		}
	}

	async fn ready(&self, context: Context, _ready: Ready) {
		println!("Ready");
		let arg = std::env::args().nth(1);
		if let Some(arg) = arg {
			if &arg == "register" {
				for guild in context.cache.guilds() {
					let commands = vec![
						inventory::view::register(),
						inventory::group::register(),
						find_emoji::register(),
						trading::trade::register(),
						trading::recycling::register(),
						user_settings::private::register(),
						images::rasterize::register(),
						images::generate::register(),
						images::generate::register_test(),
					];
					let commands = guild.set_commands(&context.http, commands).await.unwrap();

					let command_names = commands.into_iter().map(|command| command.name).join(", ");
					println!(
						"I now have the following guild slash commands in guild {}: {}",
						guild.get(),
						command_names
					);
				}
			}
		}
	}
}
