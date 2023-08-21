use itertools::Itertools;
use serenity::{
	async_trait,
	model::prelude::{application_command::ApplicationCommandInteraction, *},
	prelude::*,
};
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
	async fn handle_application_command(
		&self,
		context: Context,
		interaction: ApplicationCommandInteraction,
	) {
		match interaction.data.name.as_str() {
			"inventory" => {
				inventory::view::execute(&self.database, &self.emoji_map, context, interaction)
					.await;
			}
			"group" => {
				inventory::group::execute(&self.database, &self.emoji_map, context, interaction)
					.await;
			}
			"who" => {
				find_emoji::execute(&self.database, &self.emoji_map, context, interaction).await;
			}
			"trade" => {
				trading::trade::execute(&self.database, &self.emoji_map, context, interaction)
					.await;
			}
			"recycle" => {
				trading::recycling::execute(&self.database, &self.emoji_map, context, interaction)
					.await
			}
			"private" => {
				user_settings::private::execute(&self.database, context, interaction).await;
			}
			"image" => {
				images::rasterize::execute(&self.database, &self.emoji_map, context, interaction)
					.await;
			}
			"generate" => {
				images::generate::execute(&self.database, &self.emoji_map, context, interaction)
					.await;
			}
			"testimage" => {
				images::generate::execute_test(&self.emoji_map, context, interaction).await;
			}
			_ => (),
		};
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
		if let Interaction::ApplicationCommand(interaction) = interaction {
			self.handle_application_command(context, interaction).await;
		}
	}

	async fn ready(&self, context: Context, _ready: Ready) {
		println!("Ready");
		let arg = std::env::args().nth(1);
		if let Some(arg) = arg {
			if &arg == "register" {
				for guild in context.cache.guilds() {
					let commands = guild
						.set_application_commands(&context.http, |commands| {
							commands
								.create_application_command(inventory::view::register)
								.create_application_command(inventory::group::register)
								.create_application_command(find_emoji::register)
								.create_application_command(trading::trade::register)
								.create_application_command(trading::recycling::register)
								.create_application_command(user_settings::private::register)
								.create_application_command(images::rasterize::register)
								.create_application_command(images::generate::register)
								.create_application_command(images::generate::register_test)
						})
						.await
						.unwrap();

					let command_names = commands.into_iter().map(|command| command.name).join(", ");
					println!(
						"I now have the following guild slash commands in guild {}: {}",
						guild.as_u64(),
						command_names
					);
				}
			}
		}
	}
}
